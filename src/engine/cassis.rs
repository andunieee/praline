//! Paying and receiving Cassis invoices on the mints' networks.
//!
//! A mint's Cassis network (`cashu::<host>`) is served by the mint's own cdk wallet: it locks the
//! HTLCs of payments and claims the ones it is paid (see [`htlc::CdkWallet`]), so the wallet's
//! ecash is all there is.

use std::{collections::HashSet, sync::Arc};

use cassis_core::{Invoice, NetworkId, routing::RouteAnnouncement, sats_for_msat};
use serde_json::Value;

use super::{
    Engine,
    htlc::CdkWallet,
    invoice,
    mints::host_of,
    store::{Blob, BlobKey, CassisDirection, CassisStatus, CassisTx, CopSessionId, RouteHop, unix_now},
};

/// What paying an invoice would do, for the wallet's payment screen. Everything here comes from
/// the invoice itself, never from the page it was sent with.
#[derive(Clone, Debug)]
pub struct PaymentOffer {
    pub(crate) invoice: Invoice,
    pub amount_sat: u64,
    /// The payee's key.
    pub payee: String,
    pub description: Option<String>,
    pub expires_at: u64,
    /// The mints it can be paid from, best first.
    pub options: Vec<PayOption>,
}

impl PartialEq for PaymentOffer {
    fn eq(&self, other: &Self) -> bool {
        self.invoice.body.payment_hash == other.invoice.body.payment_hash
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PayOption {
    pub mint_url: String,
    pub mint_name: String,
    pub balance_sat: u64,
    /// Whether the balance covers the invoice with room for the mint's fees.
    pub enough: bool,
}

/// What the history keeps of a payment besides the invoice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaymentNotes {
    /// Who it is paid to, as the server called itself.
    pub peer: Option<String>,
    /// Shown in place of the invoice's description.
    pub memo: Option<String>,
    /// The COP session it is paid in.
    pub cop_session: Option<String>,
}

/// An invoice that was paid.
#[derive(Clone, Debug, PartialEq)]
pub struct PaidInvoice {
    pub amount_sat: u64,
    pub fee_sat: u64,
}

/// The Cassis network of a mint, if it has one: only mints at the root of their host do.
pub(crate) fn cassis_network(mint_url: &str) -> Option<NetworkId> {
    let host = host_of(mint_url);
    if host.contains('/') {
        return None;
    }
    let id = NetworkId::parse(&format!("cashu::{host}")).ok()?;
    let back = id.host_url().ok()?;
    (back.trim_end_matches('/') == mint_url.trim_end_matches('/')).then_some(id)
}

/// The nostr relays routes are read from, until the user picks their own.
pub const DEFAULT_RELAYS: &[&str] = &[
    "wss://nos.lol",
    "wss://relay.ditto.pub",
    "wss://relay.primal.net",
    "wss://nostr.wine",
];

const RELAYS_KEY: &str = "setting/relays";

/// A relay as typed, as a `wss://` URL. A bare host gets `wss://`; any scheme but `ws` and `wss`
/// is not a relay.
pub fn normalize_relay(relay: &str) -> Option<String> {
    let relay = relay.trim();
    if relay.contains(char::is_whitespace) {
        return None;
    }
    let (scheme, host) = relay.split_once("://").unwrap_or(("wss", relay));
    let host = host.trim_end_matches('/');
    if host.is_empty() || !matches!(scheme, "wss" | "ws") {
        return None;
    }
    Some(format!("{scheme}://{host}"))
}

fn route_hop(hop: &RouteAnnouncement) -> RouteHop {
    RouteHop {
        node: hop.node_pubkey.to_string(),
        from: hop.from.0.clone(),
        to: hop.to.0.clone(),
        fee_base_msat: hop.fee_base_msat,
        fee_ppm: hop.fee_ppm,
    }
}

fn hash_bytes(hex: &str) -> Option<[u8; 32]> {
    lowercase_hex::decode(hex).ok()?.try_into().ok()
}

/// The most the mint's fees can add to paying `amount_sat`: a percent, and a few sats.
fn fee_room(amount_sat: u64) -> u64 {
    amount_sat.div_ceil(100) + 3
}

impl Engine {
    /// Whether the mint pays and is paid with Cassis.
    pub(crate) fn on_cassis(&self, mint_url: &str) -> bool {
        self.0.cassis_mints().contains(mint_url)
    }

    /// The nostr relays Cassis routes are read from.
    pub fn relays(&self) -> Vec<String> {
        self.0
            .db
            .blobs()
            .get(BlobKey(RELAYS_KEY.to_owned()))
            .ok()
            .flatten()
            .and_then(|blob| serde_json::from_slice(&blob.data).ok())
            .unwrap_or_else(|| DEFAULT_RELAYS.iter().map(|r| (*r).to_owned()).collect())
    }

    /// Keep the relays to read Cassis routes from. At least one is needed.
    pub fn set_relays(&self, mut relays: Vec<String>) -> Result<(), String> {
        let mut seen = HashSet::new();
        relays.retain(|relay| seen.insert(relay.clone()));
        if relays.is_empty() {
            return Err("Add at least one relay.".into());
        }
        let data = serde_json::to_vec(&relays).map_err(|e| e.to_string())?;
        self.0
            .db
            .blobs()
            .upsert(&Blob {
                key: BlobKey(RELAYS_KEY.to_owned()),
                kind: "setting".to_owned(),
                group: String::new(),
                data,
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Put the mint on Cassis, if it can be: its cdk wallet funds the payments and claims the
    /// ones it is paid.
    pub(crate) fn open_cassis(&self, mint_url: &str) {
        if let Err(error) = self.try_open_cassis(mint_url) {
            tracing::warn!("{mint_url} is not on Cassis: {error}");
        }
    }

    fn try_open_cassis(&self, mint_url: &str) -> Result<(), String> {
        let network = cassis_network(mint_url).ok_or("its address is not a plain host")?;
        if self.on_cassis(mint_url) {
            return Ok(());
        }
        let wallet = Arc::new(CdkWallet::new(network.clone(), self.wallet(mint_url)?, &self.0.seed));
        self.0.node.set_funder(network.clone(), wallet.clone());
        self.0.node.set_receiver(network, wallet);
        // Only now: a mint that failed to open is tried again the next time.
        self.0.cassis_mints().insert(mint_url.to_owned());
        Ok(())
    }

    /// Put every mint the wallet had on Cassis, at startup.
    pub(crate) async fn reopen_networks(self) {
        let urls = cdk::cdk_database::WalletDatabase::get_mints(&self.0.store)
            .await
            .unwrap_or_default()
            .into_keys()
            .map(|url| url.to_string());
        for url in urls {
            self.open_cassis(url.trim_end_matches('/'));
        }
    }

    /// Read an invoice a server sent and work out how it could be paid.
    pub async fn offer(&self, invoice: Value) -> Result<PaymentOffer, String> {
        let invoice: Invoice =
            serde_json::from_value(invoice).map_err(|e| format!("not a Cassis invoice: {e}"))?;
        invoice.verify()?;
        let amount_sat = sats_for_msat(invoice.body.amount_msat);
        // Any mint on Cassis can pay: if the invoice is not on its network, the payment is routed.
        let urls: Vec<String> = self.0.cassis_mints().iter().cloned().collect();
        let mut options = Vec::new();
        for url in urls {
            let summary = self.mint_summary(&url).await?;
            options.push(PayOption {
                enough: summary.balance_sat >= amount_sat + fee_room(amount_sat),
                mint_url: url,
                mint_name: summary.name,
                balance_sat: summary.balance_sat,
            });
        }
        options.sort_by_key(|o| (!o.enough, std::cmp::Reverse(o.balance_sat)));
        Ok(PaymentOffer {
            amount_sat,
            payee: invoice.receiver.payee().to_string(),
            description: invoice.body.description.clone(),
            expires_at: invoice.body.expires_at,
            invoice,
            options,
        })
    }

    /// Pay an offer from the mint at `mint_url`, noting `notes` in the history.
    pub async fn pay(
        &self,
        offer: &PaymentOffer,
        mint_url: &str,
        notes: PaymentNotes,
    ) -> Result<PaidInvoice, String> {
        let (engine, offer, mint_url) = (self.clone(), offer.clone(), mint_url.to_owned());
        self.run(async move {
            let network = cassis_network(&mint_url).ok_or("this mint is not on Cassis")?;
            let hash = offer.invoice.body.payment_hash.to_string();
            let before = engine.balance(&mint_url).await?;

            let mut row = CassisTx {
                description: notes.memo.or_else(|| offer.description.clone()),
                peer: notes.peer,
                cop_session: notes.cop_session.map(CopSessionId),
                ..CassisTx::pending(
                    hash,
                    mint_url.clone(),
                    CassisDirection::Sent,
                    offer.amount_sat,
                    offer.expires_at,
                )
            };
            engine.0.db.cassis_txes().upsert(&row).map_err(|e| e.to_string())?;

            // The HTLC is funded from the mint's cdk wallet, see [`CdkWallet`].
            let outcome = async {
                engine
                    .0
                    .node
                    .pay(&offer.invoice, &network, &engine.relays(), &HashSet::new())
                    .await
            }
            .await;

            // A payment that could not start has no route; otherwise the report says how far it got.
            let outcome = match outcome {
                Ok(report) => {
                    row.route = report.route.iter().map(route_hop).collect();
                    match report.outcome {
                        Ok(_) => Ok(()),
                        Err(failure) => {
                            if let Some(hop) = &failure.hop {
                                row.failed_hop = u32::try_from(hop.index).ok();
                                row.hop_error = Some(hop.error.to_string());
                            }
                            Err(failure.message)
                        }
                    }
                }
                Err(error) => Err(error),
            };
            match outcome {
                Ok(()) => {
                    let after = engine.balance(&mint_url).await.unwrap_or(before);
                    row.status = CassisStatus::Done;
                    row.fee_sat = before.saturating_sub(after).saturating_sub(offer.amount_sat);
                    engine.0.db.cassis_txes().upsert(&row).map_err(|e| e.to_string())?;
                    Ok(PaidInvoice {
                        amount_sat: offer.amount_sat,
                        fee_sat: row.fee_sat,
                    })
                }
                Err(error) => {
                    row.status = CassisStatus::Failed;
                    row.error = Some(error.clone());
                    engine.0.db.cassis_txes().upsert(&row).map_err(|e| e.to_string())?;
                    Err(error)
                }
            }
        })
        .await
    }

    /// The mints that can receive on one of the `networks` a server offered to pay from, with the
    /// sats it offers.
    pub async fn receivable_mints(&self, networks: &[String]) -> Result<Vec<(String, String)>, String> {
        let mut found = Vec::new();
        for summary in self.mints().await? {
            let Some(id) = cassis_network(&summary.url) else {
                continue;
            };
            if self.on_cassis(&summary.url) && networks.iter().any(|n| *n == id.0) {
                found.push((summary.url, summary.name));
            }
        }
        Ok(found)
    }

    /// Make the invoice that answers an `invoicerequest` offering `amount_sat`, payable at the
    /// mint at `mint_url`. Returns it as the JSON to send back.
    pub async fn invoice_for_offer(
        &self,
        mint_url: &str,
        amount_sat: u64,
        memo: Option<String>,
        expires_in: Option<u64>,
        peer: Option<String>,
    ) -> Result<Value, String> {
        let (engine, mint_url) = (self.clone(), mint_url.to_owned());
        self.run(async move {
            let network = cassis_network(&mint_url).ok_or("this mint is not on Cassis")?;
            let expires_at = unix_now() + expires_in.unwrap_or(15 * 60).clamp(60, 24 * 3600);
            let (metadata, preimage) = invoice::issue(
                &invoice::invoice_secret(&engine.0.seed),
                invoice::Terms {
                    amount_msat: amount_sat * 1000,
                    expires_at,
                    memo: memo.clone(),
                    peer,
                },
            )?;
            // Nothing of the invoice is kept: the metadata brings it back with the payment.
            let invoice = engine
                .0
                .node
                .build_invoice(
                    &[network],
                    preimage.sha256(),
                    amount_sat * 1000,
                    expires_at,
                    memo,
                    Some(metadata),
                )
                .await?;
            serde_json::to_value(&invoice).map_err(|e| e.to_string())
        })
        .await
    }

    /// Settle the records of Cassis payments with what the node knows of them.
    pub(crate) async fn reconcile_cassis(&self) {
        let now = unix_now();
        let Ok(rows) = self.0.db.cassis_txes().all() else {
            return;
        };
        for mut row in rows.into_iter().filter(|r| r.status == CassisStatus::Pending) {
            let done = hash_bytes(&row.payment_hash.0).is_some_and(|hash| {
                self.0
                    .node
                    .db()
                    .transactions()
                    .get(cassis_wallet::db::TransactionHash(hash))
                    .ok()
                    .flatten()
                    .is_some()
            });
            if done {
                row.status = CassisStatus::Done;
            } else if row.expires_at + 3600 < now {
                row.status = CassisStatus::Failed;
                row.error = Some("expired".into());
            } else {
                continue;
            }
            let _ = self.0.db.cassis_txes().upsert(&row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_relays() {
        assert_eq!(normalize_relay("nos.lol").as_deref(), Some("wss://nos.lol"));
        assert_eq!(normalize_relay("  wss://relay.primal.net/ ").as_deref(), Some("wss://relay.primal.net"));
        assert_eq!(normalize_relay("ws://localhost:7777").as_deref(), Some("ws://localhost:7777"));
        assert_eq!(normalize_relay("https://relay.example.com"), None);
        assert_eq!(normalize_relay("wss://"), None);
        assert_eq!(normalize_relay(""), None);
        assert_eq!(normalize_relay("nos .lol"), None);
    }

    #[test]
    fn only_mints_at_the_root_of_their_host_are_on_cassis() {
        let network = cassis_network("https://mint.example.com").unwrap();
        assert_eq!(network.0, "cashu::mint.example.com");
        assert_eq!(cassis_network("https://mint.example.com/"), Some(network));
        assert_eq!(cassis_network("https://mint.example.com/sat"), None);
        // The network names a host, and the host an https URL.
        assert_eq!(cassis_network("http://mint.example.com"), None);
    }

    #[test]
    fn decodes_payment_hashes() {
        let hash = [0xab; 32];
        assert_eq!(hash_bytes(&lowercase_hex::encode(hash)), Some(hash));
        assert_eq!(hash_bytes("abab"), None);
        assert_eq!(hash_bytes("not hex"), None);
    }
}
