use std::str::FromStr;

use cassis_core::{Htlc, NetworkId};
use cassis_networks::cashu::{HtlcTerms, decode_proofs};
use cdk::{
    cdk_database::WalletDatabase,
    mint_url::MintUrl,
    nuts::State,
};

use super::{Engine, cassis::cassis_network};

/// Where a proof is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofHolder {
    /// The cdk wallet.
    Wallet,
    /// The mint's Cassis side.
    Cassis,
}

/// A proof of a mint, as the lists show it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProofView {
    pub amount_sat: u64,
    pub keyset: String,
    pub holder: ProofHolder,
    /// Whether it is set aside for an operation that is under way.
    pub reserved: bool,
}

/// An HTLC-locked proof waiting to be claimed or refunded.
#[derive(Clone, Debug, PartialEq)]
pub struct LockedProofView {
    pub amount_sat: u64,
    pub keyset: String,
    pub payment_hash: String,
    /// Unix time after which it can be refunded.
    pub refundable_at: u64,
}

/// The proofs of a mint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MintProofs {
    pub locked: Vec<LockedProofView>,
    pub held: Vec<ProofView>,
}

impl Engine {
    /// The proofs the wallet has at a mint, biggest first, and the HTLC-locked ones pending.
    pub async fn proofs(&self, url: &str) -> Result<MintProofs, String> {
        let (engine, url) = (self.clone(), url.to_owned());
        self.run(async move {
            let mint_url = MintUrl::from_str(&url).map_err(|e| e.to_string())?;
            let mut held: Vec<ProofView> = engine
                .0
                .store
                .get_proofs(
                    Some(mint_url),
                    None,
                    Some(vec![State::Unspent, State::Reserved, State::Pending]),
                    None,
                )
                .await
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|p| ProofView {
                    amount_sat: u64::from(p.proof.amount),
                    keyset: p.proof.keyset_id.to_string(),
                    holder: ProofHolder::Wallet,
                    reserved: p.state != State::Unspent,
                })
                .collect();
            let locked = match cassis_network(&url) {
                Some(network) => engine.locked_proofs(&network)?,
                None => Vec::new(),
            };
            held.sort_by_key(|p| std::cmp::Reverse(p.amount_sat));
            Ok(MintProofs { locked, held })
        })
        .await
    }

    /// The proofs of the HTLCs the node funded on `network` and is waiting on, first refundable
    /// first. They left the cdk wallet when they were locked, so only the node knows of them.
    fn locked_proofs(&self, network: &NetworkId) -> Result<Vec<LockedProofView>, String> {
        let payments = self.0.node.db().pending_payments().all().map_err(|e| e.to_string())?;
        let mut locked = Vec::new();
        for payment in payments.into_iter().filter(|p| p.network == network.0) {
            let Ok(htlc) = Htlc::try_from(&payment.htlc) else {
                continue;
            };
            let Ok((proofs, _)) = decode_proofs(&htlc.descriptor) else {
                continue;
            };
            locked.extend(proofs.iter().map(|proof| LockedProofView {
                amount_sat: u64::from(proof.amount),
                keyset: proof.keyset_id.to_string(),
                payment_hash: htlc.payment_hash.to_string(),
                refundable_at: HtlcTerms::of(proof).map_or(htlc.expiry, |terms| terms.locktime),
            }));
        }
        locked.sort_by_key(|p| p.refundable_at);
        Ok(locked)
    }
}
