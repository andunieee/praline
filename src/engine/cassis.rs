//! Paying and receiving Cassis invoices on the mints' networks.
//!
//! A mint's Cassis network (`cashu::<host>`) keeps its own ecash, locked into HTLCs for each
//! payment, apart from the cdk wallet's. So paying moves what is needed from the cdk wallet to
//! the Cassis side first, and what is left comes back afterwards: the mint balance the UI shows
//! is the two together.

use std::{collections::HashSet, sync::Arc};

use cassis_core::{Invoice, NetworkId};
use cassis_core::routing::RouteAnnouncement;
use cassis_networks::{Wallet as NetworkWallet, cashu::CashuAdapter};
use serde_json::Value;

use super::{
    Engine,
    mints::host_of,
    store::{Blob, BlobKey, CassisDirection, CassisStatus, CassisTx, CassisTxHash, CopSessionId, RouteHop, unix_now},
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

/// A relay as typed, as a `wss://` URL.
pub fn normalize_relay(relay: &str) -> Option<String> {
    let relay = relay.trim().trim_end_matches('/');
    if relay.is_empty() || relay.contains(char::is_whitespace) {
        return None;
    }
    if relay.starts_with("wss://") || relay.starts_with("ws://") {
        Some(relay.to_owned())
    } else {
        Some(format!("wss://{relay}"))
    }
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

fn sats_ceil(msat: u64) -> u64 {
    msat.div_ceil(1000)
}

/// The most the mint's fees can add to paying `amount_sat`: a percent, and a few sats.
fn fee_room(amount_sat: u64) -> u64 {
    amount_sat.div_ceil(100) + 3
}

impl Engine {
    pub(crate) fn adapter(&self, mint_url: &str) -> Option<Arc<CashuAdapter>> {
        self.0
            .adapters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(mint_url)
            .cloned()
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
    pub fn set_relays(&self, relays: Vec<String>) -> Result<(), String> {
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

    /// What the Cassis side of a mint holds, in sats.
    pub(crate) fn float_sat(&self, mint_url: &str) -> u64 {
        self.adapter(mint_url)
            .and_then(|a| a.balance_msat().ok())
            .map_or(0, |msat| msat / 1000)
    }

    /// Put the mint on Cassis, if it can be.
    pub(crate) async fn open_cassis(&self, mint_url: &str) {
        if let Err(error) = self.try_open_cassis(mint_url).await {
            tracing::warn!("{mint_url} is not on Cassis: {error}");
        }
    }

    async fn try_open_cassis(&self, mint_url: &str) -> Result<(), String> {
        let network = cassis_network(mint_url).ok_or("its address is not a plain host")?;
        if self.adapter(mint_url).is_some() {
            return Ok(());
        }
        let settings = {
            let mut home = self.0.home.lock().unwrap_or_else(|e| e.into_inner());
            if home.config.network(&network).is_err() {
                home.enable(&network, |_| Ok(()))?;
            }
            home.settings(&network)?
        };
        let (opened, wallet) =
            cassis_networks::open_wallet(&network, &settings, &self.0.node.span()).await?;
        #[allow(irrefutable_let_patterns)]
        let NetworkWallet::Cashu(adapter) = wallet else {
            return Err("not a cashu network".into());
        };
        self.0.node.add_network(opened);
        self.0
            .adapters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(mint_url.to_owned(), adapter);
        Ok(())
    }

    /// Open the Cassis side of every mint the wallet had, at startup.
    pub(crate) async fn reopen_networks(self) {
        let ids = {
            let home = self.0.home.lock().unwrap_or_else(|e| e.into_inner());
            home.config.network_ids().unwrap_or_default()
        };
        for id in ids.into_iter().filter(|id| id.kind() == "cashu") {
            if let Ok(url) = cassis_networks::cashu::mint_url(&id) {
                self.open_cassis(url.trim_end_matches('/')).await;
            }
        }
    }

    /// Read an invoice a server sent and work out how it could be paid.
    pub async fn offer(&self, invoice: Value) -> Result<PaymentOffer, String> {
        let invoice: Invoice =
            serde_json::from_value(invoice).map_err(|e| format!("not a Cassis invoice: {e}"))?;
        invoice.verify()?;
        let amount_sat = sats_ceil(invoice.body.amount_msat);
        // Any mint on Cassis can pay: if the invoice is not on its network, the payment is routed.
        let urls: Vec<String> = self
            .0
            .adapters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
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

    /// Pay an offer from the mint at `mint_url`. `peer` is who it is paid to, as the server
    /// called itself, for the transaction list.
    pub async fn pay(
        &self,
        offer: &PaymentOffer,
        mint_url: &str,
        peer: Option<String>,
        memo: Option<String>,
        cop_session: Option<String>,
    ) -> Result<PaidInvoice, String> {
        let (engine, offer, mint_url) = (self.clone(), offer.clone(), mint_url.to_owned());
        self.run(async move {
            let network = cassis_network(&mint_url).ok_or("this mint is not on Cassis")?;
            let hash = offer.invoice.body.payment_hash.to_string();
            let before = engine.balance(&mint_url).await?;

            let row = CassisTx {
                payment_hash: CassisTxHash(hash.clone()),
                mint_url: mint_url.clone(),
                direction: CassisDirection::Sent,
                amount_sat: offer.amount_sat,
                fee_sat: 0,
                description: memo.or_else(|| offer.description.clone()),
                peer,
                status: CassisStatus::Pending,
                error: None,
                route: Vec::new(),
                failed_hop: None,
                hop_error: None,
                cop_session: cop_session.map(CopSessionId),
                at: unix_now(),
                expires_at: offer.expires_at,
            };
            engine.0.db.cassis_txes().upsert(&row).map_err(|e| e.to_string())?;

            let outcome = async {
                let want = offer.amount_sat + fee_room(offer.amount_sat);
                let held = engine.float_sat(&mint_url);
                if held < want {
                    let cdk_sat = u64::from(
                        engine.wallet(&mint_url)?.total_balance().await.map_err(|e| e.to_string())?,
                    );
                    let missing = (want - held).min(cdk_sat);
                    if missing > 0 {
                        engine.fund_cassis(&mint_url, missing).await?;
                    }
                }
                engine
                    .0
                    .node
                    .pay(&offer.invoice, &network, &engine.relays(), &HashSet::new())
                    .await
            }
            .await;

            let mut row = row;
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
                    engine.consolidate(&mint_url).await;
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
                    // What was moved for it is still on the Cassis side.
                    engine.consolidate(&mint_url).await;
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
            if self.adapter(&summary.url).is_some() && networks.iter().any(|n| *n == id.0) {
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
            let invoice = engine
                .0
                .node
                .create_invoice(&[network], amount_sat * 1000, expires_at, memo.clone(), None)
                .await?;
            engine
                .0
                .db
                .cassis_txes()
                .upsert(&CassisTx {
                    payment_hash: CassisTxHash(invoice.body.payment_hash.to_string()),
                    mint_url,
                    direction: CassisDirection::Received,
                    amount_sat,
                    fee_sat: 0,
                    description: memo,
                    peer,
                    status: CassisStatus::Pending,
                    error: None,
                    route: Vec::new(),
                    failed_hop: None,
                    hop_error: None,
                    cop_session: None,
                    at: unix_now(),
                    expires_at,
                })
                .map_err(|e| e.to_string())?;
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
        let mut busy_mints = std::collections::HashSet::new();
        for mut row in rows.into_iter().filter(|r| r.status == CassisStatus::Pending) {
            busy_mints.insert(row.mint_url.clone());
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
        // Ecash left on the Cassis side (a refund, a payment received) goes back to the wallet.
        let mints: Vec<String> = self
            .0
            .adapters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        for mint in mints {
            if !busy_mints.contains(&mint) && self.float_sat(&mint) > 0 {
                self.consolidate(&mint).await;
            }
        }
    }
}
