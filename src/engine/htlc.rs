//! The HTLCs of Cassis payments, made and claimed by the cdk wallet of a mint.
//!
//! Cassis prepares a payment and hands over what to lock: the proofs are made by the same cdk
//! wallet that holds the user's ecash, locked to the receiver with a refund path for a key only
//! this wallet has. Paying it, the wallet claims the HTLCs payers lock to it the same way. No
//! ecash is ever moved to a wallet of Cassis's.

use std::sync::Arc;

use async_trait::async_trait;
use cassis_core::{
    Bytes32, Htlc, HtlcFunder, HtlcReceiver, HtlcRequest, InvoiceTarget, NetworkError, NetworkId,
    OutgoingHtlcs, OutgoingState, XOnlyPubKey, sats_for_msat,
};
use cassis_networks::cashu::{
    HtlcTerms, cashu_key, decode_proofs, encode_proofs, htlc_amount_sat, outgoing_state,
    sign_refund, verify_htlc_proofs,
};
use cdk::{
    Amount,
    amount::SplitTarget,
    nuts::{SecretKey, nut07::CheckStateRequest},
    secp256k1::hashes::{Hash, sha256},
    wallet::{ReceiveOptions, SendOptions, Wallet},
};

/// Tags the derivation of the key HTLCs are refunded with from the wallet's seed.
const REFUND_KEY_TAG: &[u8] = b"praline/cassis-htlc-refund";
/// Tags the derivation of the key payers lock HTLCs to.
const CLAIM_KEY_TAG: &[u8] = b"praline/cassis-htlc-claim";

/// A key from `seed`, with the even-Y public key that proofs locked to its x-only form carry.
fn derive_key(seed: &[u8; 64], tag: &[u8]) -> SecretKey {
    let mut tagged = tag.to_vec();
    tagged.extend_from_slice(seed);
    let digest = sha256::Hash::hash(&tagged);
    let key = SecretKey::from_slice(digest.as_byte_array())
        .expect("a hash is a valid key, but for a chance of 2^-128");
    if key.public_key().to_bytes()[0] == 0x03 {
        let negated = cdk::secp256k1::SecretKey::from_slice(&key.to_secret_bytes())
            .expect("a valid key")
            .negate();
        return SecretKey::from(negated);
    }
    key
}

pub(crate) struct CdkWallet {
    network: NetworkId,
    wallet: Arc<Wallet>,
    refund_key: SecretKey,
    claim_key: SecretKey,
}

impl CdkWallet {
    /// What funds and receives on `network` for `wallet`'s mint. The keys come from `seed`, so
    /// the recovery phrase also recovers HTLCs that were never claimed.
    pub(crate) fn new(network: NetworkId, wallet: Arc<Wallet>, seed: &[u8; 64]) -> Self {
        Self {
            network,
            wallet,
            refund_key: derive_key(seed, REFUND_KEY_TAG),
            claim_key: derive_key(seed, CLAIM_KEY_TAG),
        }
    }

    /// Where payers lock their HTLCs to.
    fn claim_target(&self) -> InvoiceTarget {
        let compressed = self.claim_key.public_key().to_bytes();
        let target = XOnlyPubKey::from_bytes(compressed[1..].try_into().expect("33 bytes"))
            .expect("a public key is an x-only key");
        InvoiceTarget::Cashu { network: self.network.clone(), target }
    }

    async fn states(
        &self,
        proofs: &[cdk::nuts::Proof],
    ) -> Result<Vec<cdk::nuts::nut07::ProofState>, NetworkError> {
        let ys = proofs
            .iter()
            .map(|proof| proof.y())
            .collect::<Result<Vec<_>, _>>()
            .map_err(NetworkError::invalid)?;
        let response = self
            .wallet
            .mint_connector()
            .post_check_state(CheckStateRequest { ys })
            .await
            .map_err(failed)?;
        if response.states.len() != proofs.len() {
            return Err(NetworkError::network("mint returned an incomplete state set"));
        }
        Ok(response.states)
    }
}

fn failed(error: impl std::fmt::Display) -> NetworkError {
    NetworkError::network(error)
}

#[async_trait]
impl OutgoingHtlcs for CdkWallet {
    fn network_id(&self) -> NetworkId {
        self.network.clone()
    }

    async fn check_outgoing(&self, htlc: &Htlc) -> Result<OutgoingState, NetworkError> {
        let (proofs, _) = decode_proofs(&htlc.descriptor)?;
        outgoing_state(&proofs, self.states(&proofs).await?)
    }

    async fn refund_outgoing(&self, htlc: &Htlc) -> Result<(), NetworkError> {
        let (mut proofs, _) = decode_proofs(&htlc.descriptor)?;
        for proof in &mut proofs {
            sign_refund(proof, &self.refund_key)?;
        }
        // The swap sends the proofs as they are, witness included.
        self.wallet
            .swap(None, SplitTarget::None, proofs, None, false, false)
            .await
            .map(|_| ())
            .map_err(failed)
    }
}

#[async_trait]
impl HtlcFunder for CdkWallet {
    async fn fund(&self, request: &HtlcRequest) -> Result<cassis_core::HtlcDescriptor, NetworkError> {
        let InvoiceTarget::Cashu { target: receiver, .. } = &request.target else {
            return Err(NetworkError::invalid("cashu HTLCs are locked to an x-only key"));
        };
        let terms = HtlcTerms {
            payment_hash: request.payment_hash,
            receiver: cashu_key(receiver)?,
            refund: self.refund_key.public_key(),
            locktime: request.expiry,
        };
        // The receiver pays the mint's input fee when claiming: lock enough for it to still net
        // the amount.
        let keyset = self.wallet.fetch_active_keyset().await.map_err(failed)?;
        let amount_sat = htlc_amount_sat(sats_for_msat(request.amount_msat), keyset.input_fee_ppk);
        let options = SendOptions {
            conditions: Some(terms.conditions()?),
            ..Default::default()
        };
        let prepared = self
            .wallet
            .prepare_send(Amount::from(amount_sat), options)
            .await
            .map_err(failed)?;
        let token = prepared.confirm(None).await.map_err(failed)?;
        let keysets = self.wallet.load_mint_keysets().await.map_err(failed)?;
        let proofs = token.proofs(&keysets).map_err(failed)?;
        encode_proofs(&proofs, *receiver)
    }
}

#[async_trait]
impl HtlcReceiver for CdkWallet {
    fn network_id(&self) -> NetworkId {
        self.network.clone()
    }

    fn incoming_delta_secs(&self) -> u64 {
        self.network.default_incoming_delta()
    }

    /// The same key whatever the payment: nothing is set up per payment.
    async fn incoming_target(
        &self,
        _payment_hash: Bytes32,
        _amount_msat: u64,
        _expiry: u64,
    ) -> Result<InvoiceTarget, NetworkError> {
        Ok(self.claim_target())
    }

    async fn cancel_incoming(&self, _payment_hash: Bytes32) -> Result<(), NetworkError> {
        Ok(())
    }

    async fn verify_incoming(&self, htlc: &Htlc) -> Result<(), NetworkError> {
        if !htlc.descriptor.pays(&self.claim_target()) {
            return Err(NetworkError::invalid("HTLC is not addressed to this wallet"));
        }
        verify_htlc_proofs(self.wallet.mint_connector().as_ref(), htlc).await
    }

    async fn claim_incoming(&self, htlc: &Htlc, preimage: Bytes32) -> Result<(), NetworkError> {
        let (proofs, _) = decode_proofs(&htlc.descriptor)?;
        let options = ReceiveOptions {
            preimages: vec![lowercase_hex::encode(preimage.0)],
            p2pk_signing_keys: vec![self.claim_key.clone()],
            ..Default::default()
        };
        // The ecash goes straight into the wallet.
        match self.wallet.receive_proofs(proofs.clone(), options, None, None).await {
            Ok(_) => Ok(()),
            // An earlier attempt whose answer was lost claimed them.
            Err(error) => match self.states(&proofs).await.and_then(|s| outgoing_state(&proofs, s)) {
                Ok(OutgoingState::Claimed(revealed)) if revealed == preimage => Ok(()),
                _ => Err(failed(error)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_keys_have_even_y_and_keep_their_x() {
        // Half of the seeds give an odd key before it is negated.
        for byte in 0..32u8 {
            let seed = [byte; 64];
            for tag in [REFUND_KEY_TAG, CLAIM_KEY_TAG] {
                let key = derive_key(&seed, tag);
                assert_eq!(key.public_key().to_bytes()[0], 0x02, "seed {byte}");

                let mut tagged = tag.to_vec();
                tagged.extend_from_slice(&seed);
                let raw = SecretKey::from_slice(sha256::Hash::hash(&tagged).as_byte_array()).unwrap();
                assert_eq!(raw.public_key().to_bytes()[1..], key.public_key().to_bytes()[1..]);
            }
        }
    }

    #[test]
    fn derived_keys_are_stable_and_apart() {
        let seed = [3; 64];
        let refund = derive_key(&seed, REFUND_KEY_TAG);
        assert_eq!(refund.to_secret_bytes(), derive_key(&seed, REFUND_KEY_TAG).to_secret_bytes());
        assert_ne!(refund.to_secret_bytes(), derive_key(&seed, CLAIM_KEY_TAG).to_secret_bytes());
        assert_ne!(refund.to_secret_bytes(), derive_key(&[4; 64], REFUND_KEY_TAG).to_secret_bytes());
    }
}
