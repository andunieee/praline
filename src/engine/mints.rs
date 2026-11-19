use std::{str::FromStr, sync::Arc};

use cdk::{
    amount::SplitTarget,
    mint_url::MintUrl,
    nuts::{nut00::KnownMethod, CurrencyUnit, PaymentMethod, Token},
    wallet::{MeltOutcome, MeltQuote, ReceiveOptions, SendMemo, SendOptions, Wallet},
    Amount,
};

use super::Engine;

/// A mint of the wallet, with what it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct MintSummary {
    pub url: String,
    pub name: String,
    /// What the wallet holds there, in sats.
    pub balance_sat: u64,
    /// Whether payments can be made through it with Cassis (`cashu::<host>`).
    pub cassis: bool,
    /// Whether the mint makes ecash from bitcoin sent to an address.
    pub onchain_deposit: bool,
    /// Whether the mint pays bitcoin addresses out of ecash.
    pub onchain_withdraw: bool,
}

/// The URL of a mint as the wallet keeps it: with its scheme, and no trailing slash.
pub fn normalize_mint_url(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("enter the mint's address".into());
    }
    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    MintUrl::from_str(&with_scheme)
        .map(|url| url.to_string())
        .map_err(|e| format!("not a mint address: {e}"))
}

impl Engine {
    /// The cdk wallet of `url`, made on first use.
    pub(crate) fn wallet(&self, url: &str) -> Result<Arc<Wallet>, String> {
        let mut wallets = self.0.wallets.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(wallet) = wallets.get(url) {
            return Ok(wallet.clone());
        }
        let wallet = Wallet::new(
            url,
            CurrencyUnit::Sat,
            Arc::new(self.0.store.clone()),
            self.0.seed,
            None,
        )
        .map_err(|e| e.to_string())?;
        let wallet = Arc::new(wallet);
        wallets.insert(url.to_owned(), wallet.clone());
        Ok(wallet)
    }

    /// Add a mint, checking that it answers. Returns it as it is listed.
    pub async fn add_mint(&self, url: &str) -> Result<MintSummary, String> {
        let url = normalize_mint_url(url)?;
        let engine = self.clone();
        self.run(async move {
            let wallet = engine.wallet(&url)?;
            let info = wallet
                .load_mint_info()
                .await
                .map_err(|e| format!("this mint does not answer: {e}"))?;
            cdk::cdk_database::WalletDatabase::add_mint(
                &engine.0.store,
                MintUrl::from_str(&url).map_err(|e| e.to_string())?,
                Some(info),
            )
            .await
            .map_err(|e| e.to_string())?;
            engine.open_cassis(&url);
            engine.mint_summary(&url).await
        })
        .await
    }

    /// The mints, in the order they were added.
    pub async fn mints(&self) -> Result<Vec<MintSummary>, String> {
        let engine = self.clone();
        self.run(async move {
            let mut urls: Vec<String> = cdk::cdk_database::WalletDatabase::get_mints(&engine.0.store)
                .await
                .map_err(|e| e.to_string())?
                .into_keys()
                .map(|url| url.to_string())
                .collect();
            urls.sort();
            let mut mints = Vec::with_capacity(urls.len());
            for url in urls {
                mints.push(engine.mint_summary(&url).await?);
            }
            Ok(mints)
        })
        .await
    }

    pub(crate) async fn mint_summary(&self, url: &str) -> Result<MintSummary, String> {
        let info = cdk::cdk_database::WalletDatabase::get_mint(
            &self.0.store,
            MintUrl::from_str(url).map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string())?;
        let onchain = PaymentMethod::Known(KnownMethod::Onchain);
        let supports = |deposit: bool| {
            info.as_ref().is_some_and(|info| {
                let unit = CurrencyUnit::Sat;
                if deposit {
                    !info.nuts.nut04.disabled && info.nuts.nut04.get_settings(&unit, &onchain).is_some()
                } else {
                    !info.nuts.nut05.disabled && info.nuts.nut05.get_settings(&unit, &onchain).is_some()
                }
            })
        };
        let (onchain_deposit, onchain_withdraw) = (supports(true), supports(false));
        let name = info
            .and_then(|info| info.name)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| host_of(url));
        Ok(MintSummary {
            url: url.to_owned(),
            name,
            onchain_deposit,
            onchain_withdraw,
            balance_sat: self.balance(url).await?,
            cassis: self.on_cassis(url),
        })
    }

    /// What the wallet holds at `url`, in sats.
    pub(crate) async fn balance(&self, url: &str) -> Result<u64, String> {
        let held = self
            .wallet(url)?
            .total_balance()
            .await
            .map_err(|e| e.to_string())?;
        Ok(u64::from(held))
    }

    /// Forget a mint. Only one that holds nothing.
    pub async fn remove_mint(&self, url: &str) -> Result<(), String> {
        if self.balance(url).await? > 0 {
            return Err("this mint still holds funds: withdraw or send them first".into());
        }
        cdk::cdk_database::WalletDatabase::remove_mint(
            &self.0.store,
            MintUrl::from_str(url).map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string())?;
        self.0.wallets.lock().unwrap_or_else(|e| e.into_inner()).remove(url);
        Ok(())
    }

    /// Ask the mint for a Lightning invoice that, once paid, becomes ecash in the wallet.
    /// Returns the invoice and the quote to wait on with [`Engine::finish_deposit`].
    pub async fn start_deposit(&self, url: &str, amount_sat: u64) -> Result<Deposit, String> {
        let (engine, url) = (self.clone(), url.to_owned());
        self.run(async move {
            let quote = engine
                .wallet(&url)?
                .mint_quote(PaymentMethod::BOLT11, Some(Amount::from(amount_sat)), None, None)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Deposit {
                quote_id: quote.id,
                invoice: quote.request,
                expiry: quote.expiry,
            })
        })
        .await
    }

    /// Wait for a deposit's invoice to be paid and take the ecash it makes. Returns the sats.
    pub async fn finish_deposit(&self, url: &str, quote_id: &str) -> Result<u64, String> {
        let (engine, url, quote_id) = (self.clone(), url.to_owned(), quote_id.to_owned());
        self.run(async move {
            let wallet = engine.wallet(&url)?;
            let quote = wallet
                .check_mint_quote_status(&quote_id)
                .await
                .map_err(|e| e.to_string())?;
            let expiry = quote.expiry;
            let wait = std::time::Duration::from_secs(expiry.saturating_sub(super::store::unix_now()).max(10));
            let proofs = wallet
                .wait_and_mint_quote(quote, SplitTarget::default(), None, wait)
                .await
                .map_err(|e| e.to_string())?;
            let total: u64 = proofs.iter().map(|p| u64::from(p.amount)).sum();
            Ok(total)
        })
        .await
    }

    /// What it would cost to pay a Lightning invoice out of the mint.
    pub async fn quote_withdrawal(&self, url: &str, invoice: &str) -> Result<Withdrawal, String> {
        let (engine, url, invoice) = (self.clone(), url.to_owned(), invoice.trim().to_owned());
        self.run(async move {
            let quote = engine
                .wallet(&url)?
                .melt_quote(PaymentMethod::BOLT11, invoice, None, None)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Withdrawal {
                quote_id: quote.id,
                amount_sat: u64::from(quote.amount),
                fee_reserve_sat: u64::from(quote.fee_reserve),
            })
        })
        .await
    }

    /// Pay the invoice of a [`Withdrawal`]. Returns the fee that was paid.
    pub async fn confirm_withdrawal(&self, url: &str, quote_id: &str) -> Result<u64, String> {
        let (engine, url, quote_id) = (self.clone(), url.to_owned(), quote_id.to_owned());
        self.run(async move {
            let wallet = engine.wallet(&url)?;
            let prepared = wallet
                .prepare_melt(&quote_id, Default::default())
                .await
                .map_err(|e| e.to_string())?;
            let finalized = prepared.confirm().await.map_err(|e| e.to_string())?;
            Ok(u64::from(finalized.fee_paid()))
        })
        .await
    }

    /// Ask the mint for a bitcoin address that, once paid, becomes ecash in the wallet.
    /// Returns the address and the quote to wait on with [`Engine::finish_onchain_deposit`].
    pub async fn start_onchain_deposit(&self, url: &str) -> Result<OnchainDeposit, String> {
        let (engine, url) = (self.clone(), url.to_owned());
        self.run(async move {
            let quote = engine
                .wallet(&url)?
                .mint_quote(PaymentMethod::Known(KnownMethod::Onchain), None, None, None)
                .await
                .map_err(|e| e.to_string())?;
            Ok(OnchainDeposit {
                quote_id: quote.id,
                address: quote.request,
                expiry: quote.expiry,
            })
        })
        .await
    }

    /// Wait for bitcoin to be confirmed at a deposit's address and take the ecash it makes.
    /// Returns the sats.
    pub async fn finish_onchain_deposit(&self, url: &str, quote_id: &str) -> Result<u64, String> {
        let (engine, url, quote_id) = (self.clone(), url.to_owned(), quote_id.to_owned());
        self.run(async move {
            let wallet = engine.wallet(&url)?;
            loop {
                let quote = wallet
                    .check_mint_quote_status(&quote_id)
                    .await
                    .map_err(|e| e.to_string())?;
                if u64::from(quote.amount_mintable()) > 0 {
                    wallet
                        .mint(&quote_id, SplitTarget::default(), None)
                        .await
                        .map_err(|e| e.to_string())?;
                    return Ok(u64::from(quote.amount_mintable()));
                }
                if quote.expiry != 0 && quote.expiry < super::store::unix_now() {
                    return Err("the address expired before any bitcoin was confirmed".into());
                }
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            }
        })
        .await
    }

    /// The ways the mint offers to pay `amount_sat` to a bitcoin address, one for each fee level.
    pub async fn quote_onchain_withdrawal(
        &self,
        url: &str,
        address: &str,
        amount_sat: u64,
    ) -> Result<Vec<OnchainOption>, String> {
        let (engine, url, address) = (self.clone(), url.to_owned(), address.trim().to_owned());
        self.run(async move {
            let quotes = engine
                .wallet(&url)?
                .quote_onchain_melt_options(&address, Amount::from(amount_sat), None)
                .await
                .map_err(|e| e.to_string())?;
            let mut options: Vec<_> = quotes
                .into_iter()
                .map(|quote| OnchainOption {
                    amount_sat: u64::from(quote.amount),
                    fee_reserve_sat: u64::from(quote.fee_reserve),
                    estimated_blocks: quote.estimated_blocks,
                    quote,
                })
                .collect();
            options.sort_by_key(|o| o.fee_reserve_sat);
            Ok(options)
        })
        .await
    }

    /// Pay to the address with the fee level of `option`. Returns the fee that was paid, or none
    /// if the payment was sent but is not settled yet.
    pub async fn confirm_onchain_withdrawal(
        &self,
        url: &str,
        option: &OnchainOption,
    ) -> Result<Option<u64>, String> {
        let (engine, url, quote) = (self.clone(), url.to_owned(), option.quote.clone());
        self.run(async move {
            let wallet = engine.wallet(&url)?;
            let quote_id = quote.id.clone();
            wallet
                .select_onchain_melt_quote(quote)
                .await
                .map_err(|e| e.to_string())?;
            let prepared = wallet
                .prepare_melt(&quote_id, Default::default())
                .await
                .map_err(|e| e.to_string())?;
            match prepared.confirm_prefer_async().await.map_err(|e| e.to_string())? {
                MeltOutcome::Paid(finalized) => Ok(Some(u64::from(finalized.fee_paid()))),
                MeltOutcome::Pending(_) => Ok(None),
            }
        })
        .await
    }

    /// Take `amount_sat` out of the mint as a token anyone can redeem.
    pub async fn send_token(
        &self,
        url: &str,
        amount_sat: u64,
        memo: Option<String>,
    ) -> Result<String, String> {
        let (engine, url) = (self.clone(), url.to_owned());
        self.run(async move {
            let options = SendOptions {
                memo: memo.filter(|m| !m.is_empty()).map(|memo| SendMemo {
                    memo,
                    include_memo: true,
                }),
                ..Default::default()
            };
            let wallet = engine.wallet(&url)?;
            let prepared = wallet
                .prepare_send(Amount::from(amount_sat), options)
                .await
                .map_err(|e| e.to_string())?;
            let token = prepared.confirm(None).await.map_err(|e| e.to_string())?;
            Ok(token.to_string())
        })
        .await
    }

    /// Redeem a token, adding its mint if the wallet does not know it. Returns the mint's URL and
    /// the sats received.
    pub async fn receive_token(&self, token: &str) -> Result<(String, u64), String> {
        let token = token.trim().to_owned();
        let parsed = Token::from_str(&token).map_err(|e| format!("not a cashu token: {e}"))?;
        let url = parsed.mint_url().map_err(|e| e.to_string())?.to_string();
        let known = self.mints().await?.iter().any(|m| m.url == url);
        if !known {
            self.add_mint(&url).await?;
        }
        let (engine, mint) = (self.clone(), url.clone());
        let received = self
            .run(async move {
                engine
                    .wallet(&mint)?
                    .receive(&token, ReceiveOptions::default())
                    .await
                    .map_err(|e| e.to_string())
            })
            .await?;
        Ok((url, u64::from(received)))
    }
}

/// A deposit waiting for its Lightning invoice to be paid.
#[derive(Clone, Debug, PartialEq)]
pub struct Deposit {
    pub quote_id: String,
    /// The BOLT11 invoice to pay.
    pub invoice: String,
    pub expiry: u64,
}

/// A deposit waiting for bitcoin to arrive at its address.
#[derive(Clone, Debug, PartialEq)]
pub struct OnchainDeposit {
    pub quote_id: String,
    pub address: String,
    /// Zero if the address does not expire.
    pub expiry: u64,
}

/// A bitcoin payment out of a mint, quoted at one fee level and not yet made.
#[derive(Clone, Debug, PartialEq)]
pub struct OnchainOption {
    pub amount_sat: u64,
    /// The most the network fee can be; what is not used is returned.
    pub fee_reserve_sat: u64,
    /// How long the mint expects the payment to take to confirm.
    pub estimated_blocks: Option<u32>,
    quote: MeltQuote,
}

/// A Lightning payment out of a mint, quoted and not yet made.
#[derive(Clone, Debug, PartialEq)]
pub struct Withdrawal {
    pub quote_id: String,
    pub amount_sat: u64,
    /// The most the Lightning fee can be; what is not used is returned.
    pub fee_reserve_sat: u64,
}

pub(crate) fn host_of(url: &str) -> String {
    url.split_once("://")
        .map_or(url, |(_, rest)| rest)
        .trim_end_matches('/')
        .to_owned()
}
