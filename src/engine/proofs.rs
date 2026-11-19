use std::str::FromStr;

use cdk::{
    cdk_database::WalletDatabase,
    mint_url::MintUrl,
    nuts::State,
};

use super::Engine;

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
            let locked: Vec<LockedProofView> = Vec::new();
            held.sort_by_key(|p| std::cmp::Reverse(p.amount_sat));
            Ok(MintProofs { locked, held })
        })
        .await
    }
}
