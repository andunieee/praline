use std::str::FromStr;

use cdk::{
    cdk_database::WalletDatabase,
    mint_url::MintUrl,
    wallet::types::TransactionDirection,
};

use super::{
    Engine,
    mints::host_of,
    store::{CassisDirection, CassisStatus},
};

/// What kind of money movement a transaction was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxKind {
    /// Lightning in, minted as ecash.
    Deposit,
    /// Ecash melted into a Lightning payment.
    Withdraw,
    TokenSent,
    TokenReceived,
    CassisSent,
    CassisReceived,
}

impl TxKind {
    pub fn label(self) -> &'static str {
        match self {
            TxKind::Deposit => "Deposit",
            TxKind::Withdraw => "Withdraw",
            TxKind::TokenSent => "Token sent",
            TxKind::TokenReceived => "Token received",
            TxKind::CassisSent => "Cassis sent",
            TxKind::CassisReceived => "Cassis received",
        }
    }

    pub fn incoming(self) -> bool {
        matches!(
            self,
            TxKind::Deposit | TxKind::TokenReceived | TxKind::CassisReceived
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxStatus {
    Pending,
    Done,
    Failed,
}

/// A transaction, as the lists show it.
#[derive(Clone, Debug, PartialEq)]
pub struct TxView {
    pub id: String,
    pub kind: TxKind,
    pub mint_url: String,
    pub mint_name: String,
    pub amount_sat: u64,
    pub fee_sat: u64,
    pub description: String,
    pub at: u64,
    pub status: TxStatus,
}

impl Engine {
    /// Every transaction, newest first; only those of `mint` if given.
    pub async fn transactions(&self, mint: Option<&str>) -> Result<Vec<TxView>, String> {
        let (engine, mint) = (self.clone(), mint.map(str::to_owned));
        self.run(async move {
            let mut names = std::collections::HashMap::new();
            for summary in engine.mints().await? {
                names.insert(summary.url, summary.name);
            }
            let name = |url: &str| names.get(url).cloned().unwrap_or_else(|| host_of(url));
            let mint_filter = mint
                .as_deref()
                .map(MintUrl::from_str)
                .transpose()
                .map_err(|e| e.to_string())?;

            let mut views = Vec::new();
            let cdk = engine
                .0
                .store
                .list_transactions(mint_filter, None, None)
                .await
                .map_err(|e| e.to_string())?;
            for t in cdk {
                let incoming = t.direction == TransactionDirection::Incoming;
                let lightning = t.quote_id.is_some();
                let onchain = t.payment_method.as_ref().is_some_and(|m| m.is_onchain());
                let (kind, fallback) = match (incoming, lightning) {
                    (true, true) if onchain => (TxKind::Deposit, "Onchain deposit"),
                    (false, true) if onchain => (TxKind::Withdraw, "Onchain payment"),
                    (true, true) => (TxKind::Deposit, "Lightning deposit"),
                    (false, true) => (TxKind::Withdraw, "Lightning payment"),
                    (true, false) => (TxKind::TokenReceived, "Cashu token"),
                    (false, false) => (TxKind::TokenSent, "Cashu token"),
                };
                let url = t.mint_url.to_string();
                views.push(TxView {
                    id: t.id().to_string(),
                    kind,
                    mint_name: name(&url),
                    mint_url: url,
                    amount_sat: u64::from(t.amount),
                    fee_sat: u64::from(t.fee),
                    description: t
                        .memo
                        .filter(|m| !m.is_empty())
                        .unwrap_or_else(|| fallback.to_owned()),
                    at: t.timestamp,
                    status: TxStatus::Done,
                });
            }

            let cassis = match &mint {
                Some(url) => engine
                    .0
                    .db
                    .cassis_txes()
                    .range_by_mint_at((url.clone(), 0)..=(url.clone(), u64::MAX)),
                None => engine.0.db.cassis_txes().all(),
            }
            .map_err(|e| e.to_string())?;
            for t in cassis {
                let kind = match t.direction {
                    CassisDirection::Sent => TxKind::CassisSent,
                    CassisDirection::Received => TxKind::CassisReceived,
                };
                let description = match (t.description, t.peer) {
                    (Some(d), Some(p)) if !d.is_empty() => format!("{p}: {d}"),
                    (Some(d), None) if !d.is_empty() => d,
                    (_, Some(p)) => p,
                    _ => "Cassis payment".to_owned(),
                };
                views.push(TxView {
                    id: t.payment_hash.0,
                    kind,
                    mint_name: name(&t.mint_url),
                    mint_url: t.mint_url,
                    amount_sat: t.amount_sat,
                    fee_sat: t.fee_sat,
                    description,
                    at: t.at,
                    status: match t.status {
                        CassisStatus::Pending => TxStatus::Pending,
                        CassisStatus::Done => TxStatus::Done,
                        CassisStatus::Failed => TxStatus::Failed,
                    },
                });
            }
            views.sort_by_key(|v| std::cmp::Reverse(v.at));
            Ok(views)
        })
        .await
    }
}
