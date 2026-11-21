//! Cassis invoices the wallet keeps no record of.
//!
//! What an invoice needs to be paid is in its metadata: a random nonce, the amount, the expiry and
//! the memo and peer of the history. The preimage is a hash of the wallet's static secret and
//! those bytes, so the payment hash commits to all of it, and no one but this wallet can work the
//! preimage out. When a PREPARE or COMMIT brings the metadata back the wallet derives the preimage
//! again and checks it against the payment hash, which is all it takes to know the invoice is its
//! own and the terms were not changed.

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use cassis_core::{Bytes32, protocol::{Commit, Metadata, PreparePayee}};
use cassis_wallet::node::{PreimageReleaser, Released};
use cdk::secp256k1::hashes::{Hash, sha256};
use serde::{Deserialize, Serialize};

use super::{
    Inner,
    cassis::{cassis_network, sats_ceil},
    store::{CassisDirection, CassisStatus, CassisTx, CassisTxHash, unix_now},
};

/// Tags the derivation of the static secret invoice preimages are made with.
const INVOICE_SECRET_TAG: &[u8] = b"praline/cassis-invoice-preimage";

/// The secret every invoice's preimage is hashed with.
pub(crate) fn invoice_secret(seed: &[u8; 64]) -> [u8; 32] {
    let mut tagged = INVOICE_SECRET_TAG.to_vec();
    tagged.extend_from_slice(seed);
    sha256::Hash::hash(&tagged).to_byte_array()
}

/// What the metadata says, after the nonce and the amount and expiry.
#[derive(Serialize, Deserialize, Default)]
struct Notes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    memo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    peer: Option<String>,
}

/// The terms of an invoice, as its metadata carries them.
pub(crate) struct Terms {
    pub amount_msat: u64,
    pub expires_at: u64,
    pub memo: Option<String>,
    pub peer: Option<String>,
}

/// An invoice's metadata and the preimage it makes: random, so no two invoices share a hash.
pub(crate) fn issue(secret: &[u8; 32], terms: Terms) -> Result<(Metadata, Bytes32), String> {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(&fastrand::u128(..).to_be_bytes());
    bytes.extend_from_slice(&terms.amount_msat.to_be_bytes());
    bytes.extend_from_slice(&terms.expires_at.to_be_bytes());
    let notes = Notes {
        memo: terms.memo,
        peer: terms.peer,
    };
    if notes.memo.is_some() || notes.peer.is_some() {
        bytes.extend(serde_json::to_vec(&notes).map_err(|e| e.to_string())?);
    }
    let preimage = preimage_of(secret, &bytes);
    Ok((Metadata::new(bytes)?, preimage))
}

fn preimage_of(secret: &[u8; 32], metadata: &[u8]) -> Bytes32 {
    let mut bytes = secret.to_vec();
    bytes.extend_from_slice(metadata);
    Bytes32(sha256::Hash::hash(&bytes).to_byte_array())
}

/// The preimage and terms of the invoice for `hash` that `metadata` belongs to, if it is one of
/// ours and has not expired.
fn open(
    secret: &[u8; 32],
    hash: &Bytes32,
    metadata: Option<&Metadata>,
) -> Option<(Bytes32, Terms)> {
    let bytes = metadata?.as_bytes();
    if bytes.len() < 32 {
        return None;
    }
    let preimage = preimage_of(secret, bytes);
    if !hash.matches_preimage(&preimage) {
        return None;
    }
    let amount_msat = u64::from_be_bytes(bytes[16..24].try_into().ok()?);
    let expires_at = u64::from_be_bytes(bytes[24..32].try_into().ok()?);
    if expires_at <= unix_now() {
        return None;
    }
    let notes: Notes = if bytes.len() > 32 {
        serde_json::from_slice(&bytes[32..]).ok()?
    } else {
        Notes::default()
    };
    Some((
        preimage,
        Terms {
            amount_msat,
            expires_at,
            memo: notes.memo,
            peer: notes.peer,
        },
    ))
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
            .cassis_mints
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|url| cassis_network(url).is_some_and(|n| n == commit.network))
            .cloned()
        else {
            return;
        };
        let _ = inner.db.cassis_txes().upsert(&CassisTx {
            payment_hash: hash,
            mint_url,
            direction: CassisDirection::Received,
            amount_sat: sats_ceil(terms.amount_msat),
            fee_sat: 0,
            description: terms.memo.clone(),
            peer: terms.peer.clone(),
            status: CassisStatus::Pending,
            error: None,
            route: Vec::new(),
            failed_hop: None,
            hop_error: None,
            cop_session: None,
            at: unix_now(),
            expires_at: terms.expires_at,
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
