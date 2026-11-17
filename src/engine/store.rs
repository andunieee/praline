//! `<data>/praline.redb`: everything the wallet keeps, in orangedb.
//!
//! * [`Db`] holds the wallet's own records: Cassis payments and the order flows it opened.
//! * [`CdkStore`] is cdk's wallet database (proofs, quotes, keysets, sagas, ...) on top of the
//!   same file: cdk ships no database of its own, only the trait, so the rows are cdk's types as
//!   JSON blobs under a kind, indexed by the kind and the mint they belong to.

pub use schema::*;

#[orangedb::schema]
pub mod schema {
    /// Which way a Cassis payment moved money.
    #[embed]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CassisDirection {
        Sent,
        Received,
    }

    #[embed]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CassisStatus {
        /// Waiting for the payee to claim it, or for the payer to pay.
        Pending,
        Done,
        Failed,
    }

    /// A payment on the Cassis network of one of the mints, as the wallet shows it.
    #[entity]
    #[derive(Clone, Debug, PartialEq)]
    #[index(mint_at: (String, u64) = |r| (r.mint_url.clone(), r.at))]
    pub struct CassisTx {
        /// Hex payment hash.
        #[id(String)]
        pub payment_hash: CassisTxHash,
        pub mint_url: String,
        pub direction: CassisDirection,
        pub amount_sat: u64,
        pub fee_sat: u64,
        pub description: Option<String>,
        /// The merchant or server it was paid to or received from, as it called itself.
        pub peer: Option<String>,
        pub status: CassisStatus,
        pub error: Option<String>,
        #[index]
        pub at: u64,
        /// When an unpaid invoice we issued stops being payable.
        pub expires_at: u64,
    }

    /// An order flow that was opened, kept so it can be reopened with the same session.
    #[entity]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Flow {
        /// The `session` string, which names the flow at its server.
        #[id(String)]
        pub session: FlowSession,
        /// The `cop://` URI it was opened with.
        pub uri: String,
        pub name: Option<String>,
        #[index]
        pub last_opened: u64,
    }

    /// A row of cdk's wallet database: `data` is the JSON of a cdk type.
    #[entity]
    #[derive(Clone, Debug, PartialEq)]
    #[index(kind_group: (String, String) = |r| (r.kind.clone(), r.group.clone()))]
    pub struct Blob {
        /// `<kind>/<id>`.
        #[id(String)]
        pub key: BlobKey,
        #[index]
        pub kind: String,
        /// What the row belongs to (the mint of a proof), or empty.
        pub group: String,
        pub data: Vec<u8>,
    }
}

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use cdk::{
    cdk_database::{Error, WalletDatabase},
    mint_url::MintUrl,
    nuts::{
        CurrencyUnit, Id, KeySet, KeySetInfo, Keys, MintInfo, PublicKey, SpendingConditions, State,
    },
    wallet::{
        MintQuote,
        types::{self, ProofInfo, Transaction, TransactionDirection, TransactionId},
    },
};
use serde::{Serialize, de::DeserializeOwned};

/// Open (or create) the database file.
pub fn open(path: &std::path::Path) -> Result<Arc<Db>, String> {
    Db::open(path)
        .map(Arc::new)
        .map_err(|e| format!("open {}: {e}", path.display()))
}

fn failed(error: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::Database(Box::new(error))
}

fn blob_key(kind: &str, id: &str) -> String {
    format!("{kind}/{id}")
}

/// cdk's wallet database, in the file of a [`Db`].
#[derive(Clone)]
pub struct CdkStore {
    db: Arc<Db>,
}

impl std::fmt::Debug for CdkStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CdkStore")
    }
}

impl CdkStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    fn put<T: Serialize>(&self, kind: &str, id: &str, group: &str, value: &T) -> Result<(), Error> {
        let data = serde_json::to_vec(value)?;
        self.db
            .blobs()
            .upsert(&Blob {
                key: BlobKey(blob_key(kind, id)),
                kind: kind.to_owned(),
                group: group.to_owned(),
                data,
            })
            .map(|_| ())
            .map_err(failed)
    }

    fn get<T: DeserializeOwned>(&self, kind: &str, id: &str) -> Result<Option<T>, Error> {
        self.db
            .blobs()
            .get(BlobKey(blob_key(kind, id)))
            .map_err(failed)?
            .map(|blob| serde_json::from_slice(&blob.data).map_err(Error::from))
            .transpose()
    }

    fn remove(&self, kind: &str, id: &str) -> Result<(), Error> {
        let key = BlobKey(blob_key(kind, id));
        if self.db.blobs().contains(key.clone()).map_err(failed)? {
            self.db.blobs().delete(key).map_err(failed)?;
        }
        Ok(())
    }

    fn all<T: DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>, Error> {
        self.db
            .blobs()
            .by_kind(kind.to_owned())
            .map_err(failed)?
            .into_iter()
            .map(|blob| serde_json::from_slice(&blob.data).map_err(Error::from))
            .collect()
    }

    fn in_group<T: DeserializeOwned>(&self, kind: &str, group: &str) -> Result<Vec<T>, Error> {
        self.db
            .blobs()
            .by_kind_group((kind.to_owned(), group.to_owned()))
            .map_err(failed)?
            .into_iter()
            .map(|blob| serde_json::from_slice(&blob.data).map_err(Error::from))
            .collect()
    }

    fn proofs(&self) -> Result<Vec<ProofInfo>, Error> {
        self.all("proof")
    }

    fn put_proof(&self, proof: &ProofInfo) -> Result<(), Error> {
        self.put(
            "proof",
            &proof.y.to_hex(),
            &proof.mint_url.to_string(),
            proof,
        )
    }
}

const MINT: &str = "mint";
const KEYSETS: &str = "keysets";
const MINT_QUOTE: &str = "mint_quote";
const MELT_QUOTE: &str = "melt_quote";
const KEYS: &str = "keys";
const TX: &str = "tx";
const SAGA: &str = "saga";
const COUNTER: &str = "counter";
const P2PK: &str = "p2pk";
const KV: &str = "kv";

#[async_trait]
impl WalletDatabase<Error> for CdkStore {
    async fn get_mint(&self, mint_url: MintUrl) -> Result<Option<MintInfo>, Error> {
        Ok(self
            .get::<Option<MintInfo>>(MINT, &mint_url.to_string())?
            .flatten())
    }

    async fn get_mints(&self) -> Result<HashMap<MintUrl, Option<MintInfo>>, Error> {
        let mut mints = HashMap::new();
        for blob in self.db.blobs().by_kind(MINT.to_owned()).map_err(failed)? {
            let url = blob.key.0.trim_start_matches("mint/").to_owned();
            let url = url.parse::<MintUrl>().map_err(|e| Error::Internal(e.to_string()))?;
            mints.insert(url, serde_json::from_slice(&blob.data)?);
        }
        Ok(mints)
    }

    async fn get_mint_keysets(&self, mint_url: MintUrl) -> Result<Option<Vec<KeySetInfo>>, Error> {
        self.get(KEYSETS, &mint_url.to_string())
    }

    async fn get_keyset_by_id(&self, keyset_id: &Id) -> Result<Option<KeySetInfo>, Error> {
        Ok(self
            .all::<Vec<KeySetInfo>>(KEYSETS)?
            .into_iter()
            .flatten()
            .find(|keyset| &keyset.id == keyset_id))
    }

    async fn get_mint_quote(&self, quote_id: &str) -> Result<Option<MintQuote>, Error> {
        self.get(MINT_QUOTE, quote_id)
    }

    async fn get_mint_quotes(&self) -> Result<Vec<MintQuote>, Error> {
        self.all(MINT_QUOTE)
    }

    async fn get_unissued_mint_quotes(&self) -> Result<Vec<MintQuote>, Error> {
        Ok(self
            .all::<MintQuote>(MINT_QUOTE)?
            .into_iter()
            .filter(|quote| {
                quote.payment_method.to_string() != "bolt11"
                    || u64::from(quote.amount_issued) == 0
            })
            .collect())
    }

    async fn get_melt_quote(&self, quote_id: &str) -> Result<Option<cdk::wallet::MeltQuote>, Error> {
        self.get(MELT_QUOTE, quote_id)
    }

    async fn get_melt_quotes(&self) -> Result<Vec<cdk::wallet::MeltQuote>, Error> {
        self.all(MELT_QUOTE)
    }

    async fn get_keys(&self, id: &Id) -> Result<Option<Keys>, Error> {
        self.get(KEYS, &id.to_string())
    }

    async fn get_proofs(
        &self,
        mint_url: Option<MintUrl>,
        unit: Option<CurrencyUnit>,
        state: Option<Vec<State>>,
        spending_conditions: Option<Vec<SpendingConditions>>,
    ) -> Result<Vec<ProofInfo>, Error> {
        let proofs = match &mint_url {
            Some(url) => self.in_group::<ProofInfo>("proof", &url.to_string())?,
            None => self.proofs()?,
        };
        Ok(proofs
            .into_iter()
            .filter(|p| p.matches_conditions(&mint_url, &unit, &state, &spending_conditions))
            .collect())
    }

    async fn get_proofs_by_ys(&self, ys: Vec<PublicKey>) -> Result<Vec<ProofInfo>, Error> {
        let mut found = Vec::new();
        for y in ys {
            if let Some(proof) = self.get("proof", &y.to_hex())? {
                found.push(proof);
            }
        }
        Ok(found)
    }

    async fn get_balance(
        &self,
        mint_url: Option<MintUrl>,
        unit: Option<CurrencyUnit>,
        state: Option<Vec<State>>,
    ) -> Result<u64, Error> {
        Ok(self
            .get_proofs(mint_url, unit, state, None)
            .await?
            .iter()
            .map(|p| u64::from(p.proof.amount))
            .sum())
    }

    async fn get_transaction(&self, id: TransactionId) -> Result<Option<Transaction>, Error> {
        self.get(TX, &id.to_string())
    }

    async fn list_transactions(
        &self,
        mint_url: Option<MintUrl>,
        direction: Option<TransactionDirection>,
        unit: Option<CurrencyUnit>,
    ) -> Result<Vec<Transaction>, Error> {
        let mut transactions: Vec<Transaction> = match &mint_url {
            Some(url) => self.in_group(TX, &url.to_string())?,
            None => self.all(TX)?,
        };
        transactions.retain(|t| t.matches_conditions(&mint_url, &direction, &unit));
        transactions.sort_by_key(|t| std::cmp::Reverse(t.timestamp));
        Ok(transactions)
    }

    async fn update_proofs(
        &self,
        added: Vec<ProofInfo>,
        removed_ys: Vec<PublicKey>,
    ) -> Result<(), Error> {
        for y in removed_ys {
            self.remove("proof", &y.to_hex())?;
        }
        for proof in &added {
            self.put_proof(proof)?;
        }
        Ok(())
    }

    async fn update_proofs_state(&self, ys: Vec<PublicKey>, state: State) -> Result<(), Error> {
        for y in ys {
            if let Some(mut proof) = self.get::<ProofInfo>("proof", &y.to_hex())? {
                proof.state = state;
                self.put_proof(&proof)?;
            }
        }
        Ok(())
    }

    async fn add_transaction(&self, transaction: Transaction) -> Result<(), Error> {
        self.put(
            TX,
            &transaction.id().to_string(),
            &transaction.mint_url.to_string(),
            &transaction,
        )
    }

    async fn update_mint_url(&self, old: MintUrl, new: MintUrl) -> Result<(), Error> {
        let (old_key, new_key) = (old.to_string(), new.to_string());
        if let Some(info) = self.get::<Option<MintInfo>>(MINT, &old_key)? {
            self.remove(MINT, &old_key)?;
            self.put(MINT, &new_key, "", &info)?;
        }
        if let Some(keysets) = self.get::<Vec<KeySetInfo>>(KEYSETS, &old_key)? {
            self.remove(KEYSETS, &old_key)?;
            self.put(KEYSETS, &new_key, "", &keysets)?;
        }
        for mut proof in self.in_group::<ProofInfo>("proof", &old_key)? {
            proof.mint_url = new.clone();
            self.put_proof(&proof)?;
        }
        for mut transaction in self.in_group::<Transaction>(TX, &old_key)? {
            self.remove(TX, &transaction.id().to_string())?;
            transaction.mint_url = new.clone();
            self.add_transaction(transaction).await?;
        }
        Ok(())
    }

    async fn increment_keyset_counter(&self, keyset_id: &Id, count: u32) -> Result<u32, Error> {
        let id = keyset_id.to_string();
        let next = self.get::<u32>(COUNTER, &id)?.unwrap_or(0) + count;
        self.put(COUNTER, &id, "", &next)?;
        Ok(next)
    }

    async fn add_mint(&self, mint_url: MintUrl, mint_info: Option<MintInfo>) -> Result<(), Error> {
        self.put(MINT, &mint_url.to_string(), "", &mint_info)
    }

    async fn remove_mint(&self, mint_url: MintUrl) -> Result<(), Error> {
        let key = mint_url.to_string();
        self.remove(MINT, &key)?;
        self.remove(KEYSETS, &key)
    }

    async fn add_mint_keysets(
        &self,
        mint_url: MintUrl,
        keysets: Vec<KeySetInfo>,
    ) -> Result<(), Error> {
        let key = mint_url.to_string();
        let mut known: Vec<KeySetInfo> = self.get(KEYSETS, &key)?.unwrap_or_default();
        for keyset in keysets {
            match known.iter_mut().find(|k| k.id == keyset.id) {
                Some(held) => *held = keyset,
                None => known.push(keyset),
            }
        }
        self.put(KEYSETS, &key, "", &known)
    }

    async fn add_mint_quote(&self, quote: MintQuote) -> Result<(), Error> {
        self.put(MINT_QUOTE, &quote.id, &quote.mint_url.to_string(), &quote)
    }

    async fn remove_mint_quote(&self, quote_id: &str) -> Result<(), Error> {
        self.remove(MINT_QUOTE, quote_id)
    }

    async fn add_melt_quote(&self, quote: cdk::wallet::MeltQuote) -> Result<(), Error> {
        let group = quote.mint_url.as_ref().map(|u| u.to_string()).unwrap_or_default();
        self.put(MELT_QUOTE, &quote.id, &group, &quote)
    }

    async fn remove_melt_quote(&self, quote_id: &str) -> Result<(), Error> {
        self.remove(MELT_QUOTE, quote_id)
    }

    async fn add_keys(&self, keyset: KeySet) -> Result<(), Error> {
        self.put(KEYS, &keyset.id.to_string(), "", &keyset.keys)
    }

    async fn remove_keys(&self, id: &Id) -> Result<(), Error> {
        self.remove(KEYS, &id.to_string())
    }

    async fn remove_transaction(&self, id: TransactionId) -> Result<(), Error> {
        self.remove(TX, &id.to_string())
    }

    async fn add_saga(&self, saga: types::WalletSaga) -> Result<(), Error> {
        let id = saga.id.to_string();
        if self.get::<types::WalletSaga>(SAGA, &id)?.is_some() {
            return Err(Error::Duplicate);
        }
        self.put(SAGA, &id, &saga.mint_url.to_string(), &saga)
    }

    async fn get_saga(&self, id: &uuid::Uuid) -> Result<Option<types::WalletSaga>, Error> {
        self.get(SAGA, &id.to_string())
    }

    /// The caller has already counted the new version (`WalletSaga::update_state`), so the
    /// update applies only on top of the one before it.
    async fn update_saga(&self, saga: types::WalletSaga) -> Result<bool, Error> {
        let id = saga.id.to_string();
        match self.get::<types::WalletSaga>(SAGA, &id)? {
            Some(stored) if stored.version + 1 == saga.version => {
                self.put(SAGA, &id, &saga.mint_url.to_string(), &saga)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn delete_saga(&self, id: &uuid::Uuid) -> Result<(), Error> {
        self.remove(SAGA, &id.to_string())
    }

    async fn get_incomplete_sagas(&self) -> Result<Vec<types::WalletSaga>, Error> {
        self.all(SAGA)
    }

    async fn reserve_proofs(&self, ys: Vec<PublicKey>, operation: &uuid::Uuid) -> Result<(), Error> {
        let mut proofs = Vec::with_capacity(ys.len());
        for y in &ys {
            let proof: ProofInfo = self
                .get("proof", &y.to_hex())?
                .ok_or(Error::ProofNotFound)?;
            if proof.state != State::Unspent {
                return Err(Error::ProofNotUnspent);
            }
            proofs.push(proof);
        }
        for mut proof in proofs {
            proof.state = State::Reserved;
            proof.used_by_operation = Some(*operation);
            self.put_proof(&proof)?;
        }
        Ok(())
    }

    async fn release_proofs(&self, operation: &uuid::Uuid) -> Result<(), Error> {
        for mut proof in self.get_reserved_proofs(operation).await? {
            proof.state = State::Unspent;
            proof.used_by_operation = None;
            self.put_proof(&proof)?;
        }
        Ok(())
    }

    async fn get_reserved_proofs(&self, operation: &uuid::Uuid) -> Result<Vec<ProofInfo>, Error> {
        Ok(self
            .proofs()?
            .into_iter()
            .filter(|p| p.used_by_operation.as_ref() == Some(operation))
            .collect())
    }

    async fn reserve_melt_quote(&self, quote_id: &str, operation: &uuid::Uuid) -> Result<(), Error> {
        let mut quote: cdk::wallet::MeltQuote = self
            .get(MELT_QUOTE, quote_id)?
            .ok_or(Error::QuoteNotFound)?;
        if quote.used_by_operation.is_some() {
            return Err(Error::QuoteAlreadyInUse);
        }
        quote.used_by_operation = Some(operation.to_string());
        self.add_melt_quote(quote).await
    }

    async fn release_melt_quote(&self, operation: &uuid::Uuid) -> Result<(), Error> {
        let operation = Some(operation.to_string());
        for mut quote in self.get_melt_quotes().await? {
            if quote.used_by_operation == operation {
                quote.used_by_operation = None;
                self.add_melt_quote(quote).await?;
            }
        }
        Ok(())
    }

    async fn reserve_mint_quote(&self, quote_id: &str, operation: &uuid::Uuid) -> Result<(), Error> {
        let mut quote: MintQuote = self
            .get(MINT_QUOTE, quote_id)?
            .ok_or(Error::QuoteNotFound)?;
        if quote.used_by_operation.is_some() {
            return Err(Error::QuoteAlreadyInUse);
        }
        quote.used_by_operation = Some(operation.to_string());
        self.add_mint_quote(quote).await
    }

    async fn release_mint_quote(&self, operation: &uuid::Uuid) -> Result<(), Error> {
        let operation = Some(operation.to_string());
        for mut quote in self.get_mint_quotes().await? {
            if quote.used_by_operation == operation {
                quote.used_by_operation = None;
                self.add_mint_quote(quote).await?;
            }
        }
        Ok(())
    }

    async fn kv_read(&self, primary: &str, secondary: &str, key: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self
            .db
            .blobs()
            .get(BlobKey(blob_key(KV, &format!("{primary}/{secondary}/{key}"))))
            .map_err(failed)?
            .map(|blob| blob.data))
    }

    async fn kv_list(&self, primary: &str, secondary: &str) -> Result<Vec<String>, Error> {
        let prefix = blob_key(KV, &format!("{primary}/{secondary}/"));
        Ok(self
            .db
            .blobs()
            .by_kind(KV.to_owned())
            .map_err(failed)?
            .into_iter()
            .filter_map(|blob| blob.key.0.strip_prefix(&prefix).map(str::to_owned))
            .collect())
    }

    async fn kv_write(
        &self,
        primary: &str,
        secondary: &str,
        key: &str,
        value: &[u8],
    ) -> Result<(), Error> {
        self.db
            .blobs()
            .upsert(&Blob {
                key: BlobKey(blob_key(KV, &format!("{primary}/{secondary}/{key}"))),
                kind: KV.to_owned(),
                group: String::new(),
                data: value.to_vec(),
            })
            .map(|_| ())
            .map_err(failed)
    }

    async fn kv_remove(&self, primary: &str, secondary: &str, key: &str) -> Result<(), Error> {
        self.remove(KV, &format!("{primary}/{secondary}/{key}"))
    }

    async fn add_p2pk_key(
        &self,
        pubkey: &PublicKey,
        derivation_path: bitcoin::bip32::DerivationPath,
        derivation_index: u32,
    ) -> Result<(), Error> {
        self.put(
            P2PK,
            &pubkey.to_hex(),
            "",
            &types::P2PKSigningKey {
                pubkey: *pubkey,
                derivation_path,
                derivation_index,
                created_time: unix_now(),
            },
        )
    }

    async fn get_p2pk_key(&self, pubkey: &PublicKey) -> Result<Option<types::P2PKSigningKey>, Error> {
        self.get(P2PK, &pubkey.to_hex())
    }

    async fn list_p2pk_keys(&self) -> Result<Vec<types::P2PKSigningKey>, Error> {
        self.all(P2PK)
    }

    async fn latest_p2pk(&self) -> Result<Option<types::P2PKSigningKey>, Error> {
        Ok(self
            .all::<types::P2PKSigningKey>(P2PK)?
            .into_iter()
            .max_by_key(|key| (key.created_time, key.derivation_index)))
    }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
