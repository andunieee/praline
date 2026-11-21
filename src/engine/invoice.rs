//! Cassis invoices the wallet keeps no record of.
//!
//! What an invoice needs to be paid is in its metadata: a version byte, a random nonce, the amount,
//! the expiry and the memo and peer of the history. The preimage is an HMAC of those bytes under the
//! wallet's static secret, so the payment hash commits to all of it, and no one but this wallet can
//! work the preimage out. When a PREPARE or COMMIT brings the metadata back the wallet derives the
//! preimage again and checks it against the payment hash, which is all it takes to know the invoice
//! is its own and the terms were not changed.

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use cassis_core::{Bytes32, protocol::{Commit, Metadata, PreparePayee}, sats_for_msat};
use cassis_wallet::node::{PreimageReleaser, Released};
use cdk::secp256k1::hashes::{Hash, HashEngine, hmac, sha256};
use serde::{Deserialize, Serialize};

use super::{
    Inner,
    cassis::cassis_network,
    store::{CassisDirection, CassisTx, CassisTxHash, unix_now},
};

/// Tags the derivation of the static secret invoice preimages are made with.
const INVOICE_SECRET_TAG: &[u8] = b"praline/cassis-invoice-preimage";

/// The layout of the metadata, its first byte.
const VERSION: u8 = 1;
/// The version, the nonce, the amount and the expiry; the notes follow.
const HEADER_LEN: usize = 1 + 16 + 8 + 8;

/// The secret every invoice's preimage is keyed with.
pub(crate) fn invoice_secret(seed: &[u8; 64]) -> [u8; 32] {
    let mut tagged = INVOICE_SECRET_TAG.to_vec();
    tagged.extend_from_slice(seed);
    sha256::Hash::hash(&tagged).to_byte_array()
}

/// What the metadata says, after the header.
#[derive(Serialize, Deserialize, Default)]
struct Notes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    memo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    peer: Option<String>,
}

/// The terms of an invoice, as its metadata carries them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Terms {
    pub amount_msat: u64,
    pub expires_at: u64,
    pub memo: Option<String>,
    pub peer: Option<String>,
}

fn encode(nonce: [u8; 16], terms: Terms) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(HEADER_LEN);
    bytes.push(VERSION);
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&terms.amount_msat.to_be_bytes());
    bytes.extend_from_slice(&terms.expires_at.to_be_bytes());
    let notes = Notes {
        memo: terms.memo,
        peer: terms.peer,
    };
    if notes.memo.is_some() || notes.peer.is_some() {
        bytes.extend(serde_json::to_vec(&notes).map_err(|e| e.to_string())?);
    }
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Option<Terms> {
    if bytes.len() < HEADER_LEN || bytes[0] != VERSION {
        return None;
    }
    let amount_msat = u64::from_be_bytes(bytes[17..25].try_into().ok()?);
    let expires_at = u64::from_be_bytes(bytes[25..33].try_into().ok()?);
    let notes: Notes = match &bytes[HEADER_LEN..] {
        [] => Notes::default(),
        json => serde_json::from_slice(json).ok()?,
    };
    Some(Terms {
        amount_msat,
        expires_at,
        memo: notes.memo,
        peer: notes.peer,
    })
}

/// An invoice's metadata and the preimage it makes: random, so no two invoices share a hash.
pub(crate) fn issue(secret: &[u8; 32], terms: Terms) -> Result<(Metadata, Bytes32), String> {
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).map_err(|e| format!("no randomness: {e}"))?;
    let bytes = encode(nonce, terms)?;
    let preimage = preimage_of(secret, &bytes);
    Ok((Metadata::new(bytes)?, preimage))
}

fn preimage_of(secret: &[u8; 32], metadata: &[u8]) -> Bytes32 {
    let mut engine = hmac::HmacEngine::<sha256::Hash>::new(secret);
    engine.input(metadata);
    Bytes32(hmac::Hmac::<sha256::Hash>::from_engine(engine).to_byte_array())
}

/// The preimage and terms of the invoice for `hash` that `metadata` belongs to, if it is one of
/// ours and has not expired.
fn open(
    secret: &[u8; 32],
    hash: &Bytes32,
    metadata: Option<&Metadata>,
) -> Option<(Bytes32, Terms)> {
    let bytes = metadata?.as_bytes();
    let preimage = preimage_of(secret, bytes);
    if !hash.matches_preimage(&preimage) {
        return None;
    }
    let terms = decode(bytes)?;
    if terms.expires_at <= unix_now() {
        return None;
    }
    Some((preimage, terms))
}

/// Answers the PREPARE and COMMIT of the invoices made with [`issue`].
pub(crate) struct Releaser {
    pub engine: Weak<Inner>,
    pub secret: [u8; 32],
}

#[async_trait]
impl PreimageReleaser for Releaser {
    async fn accept(&self, prepare: &PreparePayee) -> bool {
        open(&self.secret, &prepare.payment_hash, prepare.metadata.as_ref()).is_some()
    }

    async fn release(&self, commit: &Commit) -> Option<Released> {
        let (preimage, terms) = open(&self.secret, &commit.payment_hash, commit.metadata.as_ref())?;
        self.record(commit, &terms);
        Some(Released {
            preimage,
            amount_msat: terms.amount_msat,
        })
    }
}

impl Releaser {
    /// Note in the history that a payment is coming in, once. The node settles it when it claims.
    fn record(&self, commit: &Commit, terms: &Terms) {
        let Some(inner) = self.engine.upgrade() else {
            return;
        };
        let hash = CassisTxHash(commit.payment_hash.to_string());
        if inner.db.cassis_txes().get(hash.clone()).ok().flatten().is_some() {
            return;
        }
        let Some(mint_url) = inner
            .cassis_mints()
            .iter()
            .find(|url| cassis_network(url).is_some_and(|n| n == commit.network))
            .cloned()
        else {
            return;
        };
        let _ = inner.db.cassis_txes().upsert(&CassisTx {
            description: terms.memo.clone(),
            peer: terms.peer.clone(),
            ..CassisTx::pending(
                hash.0,
                mint_url,
                CassisDirection::Received,
                sats_for_msat(terms.amount_msat),
                terms.expires_at,
            )
        });
    }
}

/// Where the releaser is set, for the engine to call when it starts.
pub(crate) fn releaser(inner: &Arc<Inner>) -> Arc<Releaser> {
    Arc::new(Releaser {
        engine: Arc::downgrade(inner),
        secret: invoice_secret(&inner.seed),
    })
}
