//! Everything the wallet does, without a UI: the mints and their cdk wallets, the Cassis node
//! that pays and receives invoices on their networks, and the database under both.
//!
//! The UI calls [`Engine`] methods from its own executor; the work runs on the engine's tokio
//! runtime (cdk, iroh and reqwest need one) through [`Engine::run`].

mod cassis;
mod mints;
pub mod store;
mod transactions;

use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use cassis_networks::cashu::CashuAdapter;
use cassis_wallet::{Home, Node, NodeConfig};
use cdk::wallet::Wallet;
use store::{CdkStore, Db};

pub use cassis::{DEFAULT_RELAYS, PaymentOffer, normalize_relay};
pub use mints::{Deposit, MintSummary, OnchainDeposit, OnchainOption, Withdrawal};
pub use transactions::{TxKind, TxStatus, TxView};

/// The memo of the tokens the wallet sends itself, between its cdk wallet and the Cassis
/// network of the same mint. Hidden from the transactions: it is not money moving.
pub(crate) const FLOAT_MEMO: &str = "praline:float";

/// A handle to the wallet. Cheap to clone.
#[derive(Clone)]
pub struct Engine(Arc<Inner>);

struct Inner {
    /// Not dropped in place: a runtime cannot be dropped from inside an async context.
    runtime: std::mem::ManuallyDrop<tokio::runtime::Runtime>,
    dir: PathBuf,
    db: Arc<Db>,
    store: CdkStore,
    seed: [u8; 64],
    home: Mutex<Home>,
    node: Node,
    /// The cdk wallet of each mint, by URL.
    wallets: Mutex<HashMap<String, Arc<Wallet>>>,
    /// The Cassis side of each mint that has one, by URL: its ecash held for payments.
    adapters: Mutex<HashMap<String, Arc<CashuAdapter>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // SAFETY: taken once, here, and the field is never used again.
        unsafe { std::mem::ManuallyDrop::take(&mut self.runtime) }.shutdown_background();
    }
}

impl PartialEq for Engine {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Engine {
    /// Open the wallet in `dir`, creating it (with a new recovery phrase) on first use.
    pub fn start(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("praline")
            .build()
            .map_err(|e| format!("start the runtime: {e}"))?;

        let db = store::open(&dir.join("praline.redb"))?;
        let home = Home::open(Some(dir.join("cassis")))?;
        let seed = bip39::Mnemonic::parse(&home.config.mnemonic)
            .map_err(|e| format!("recovery phrase: {e}"))?
            .to_seed("");
        let node = runtime.block_on(Node::start(
            NodeConfig {
                keys: home.keys.clone(),
                db: home.db.clone(),
                span: tracing::info_span!("praline"),
                serve: true,
            },
            Vec::new(),
        ))?;

        let engine = Self(Arc::new(Inner {
            runtime: std::mem::ManuallyDrop::new(runtime),
            dir: dir.to_path_buf(),
            store: CdkStore::new(db.clone()),
            db,
            seed,
            home: Mutex::new(home),
            node,
            wallets: Mutex::default(),
            adapters: Mutex::default(),
        }));
        engine.run_blocking(engine.clone().reopen_networks());
        engine.spawn_upkeep();
        Ok(engine)
    }

    pub fn db(&self) -> &Arc<Db> {
        &self.0.db
    }

    pub fn data_dir(&self) -> &Path {
        &self.0.dir
    }

    /// The twelve words that recover the wallet's keys.
    pub fn recovery_phrase(&self) -> String {
        self.0.home.lock().unwrap_or_else(|e| e.into_inner()).config.mnemonic.clone()
    }

    /// The runtime, for what needs one of its own (the order flows).
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.0.runtime.handle().clone()
    }

    /// Run `future` on the engine's runtime. The returned future can be awaited from any
    /// executor, and the work goes on if it is dropped.
    pub async fn run<T: Send + 'static>(
        &self,
        future: impl Future<Output = Result<T, String>> + Send + 'static,
    ) -> Result<T, String> {
        self.0
            .runtime
            .spawn(future)
            .await
            .map_err(|e| format!("the task failed: {e}"))?
    }

    fn run_blocking<T>(&self, future: impl Future<Output = T>) -> T {
        self.0.runtime.block_on(future)
    }

    /// Housekeeping that goes on while the app is open: Cassis payments settling, paid deposits
    /// being claimed.
    fn spawn_upkeep(&self) {
        let engine = self.clone();
        self.0.runtime.spawn(async move {
            loop {
                engine.upkeep().await;
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
    }

    async fn upkeep(&self) {
        self.reconcile_cassis().await;
    }
}

impl Engine {
    /// The wallet's identity on Cassis: the iroh key payers reach it by.
    pub fn node_id(&self) -> String {
        self.0.node.iroh_peer_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against a public test mint, which pays its own invoices. Run with
    /// `cargo test --lib -- --ignored --nocapture engine_smoke`.
    #[test]
    #[ignore = "needs the network"]
    fn engine_smoke() {
        let dir = std::env::temp_dir().join(format!("praline-smoke-{}", store::unix_now()));
        let engine = Engine::start(&dir).expect("start");
        let url = "https://testnut.cashudevkit.org";
        let mint = engine.run_blocking(engine.add_mint(url)).expect("add mint");
        println!("added {mint:?}");

        let deposit = engine
            .run_blocking(engine.start_deposit(&mint.url, 100))
            .expect("quote");
        println!("pay {}", deposit.invoice);
        let got = engine
            .run_blocking(engine.finish_deposit(&mint.url, &deposit.quote_id))
            .expect("deposit");
        assert_eq!(got, 100);

        let token = engine
            .run_blocking(engine.send_token(&mint.url, 30, Some("hi".into())))
            .expect("send");
        println!("token {token}");
        let balance = engine.run_blocking(engine.balance(&mint.url)).unwrap();
        assert!(balance <= 70, "{balance}");
        let (_, back) = engine.run_blocking(engine.receive_token(&token)).expect("receive");
        assert!((29..=30).contains(&back), "{back}");

        let txs = engine.run_blocking(engine.transactions(None)).unwrap();
        for tx in &txs {
            println!("{:?}", tx);
        }
        assert!(txs.len() >= 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn find(tree: &[hypereffect::client::ResolvedTree], id: &str) -> bool {
        tree.iter().any(|n| n.id == id || find(&n.children, id))
    }

    /// Pays an order of `cassis-wallet order-demo`, whose `cop://` URI is in `COP_URI`, from a
    /// fresh wallet funded at the test mint, then answers its refund offer.
    #[test]
    #[ignore = "needs the network and a running order-demo"]
    fn cop_smoke() {
        use freya_cop::{CopEvent, CopSession, CopUri};
        use freya_hypereffect::HyperHost;

        let uri = CopUri::parse(&std::env::var("COP_URI").expect("COP_URI")).expect("uri");
        let dir = std::env::temp_dir().join(format!("praline-cop-{}", store::unix_now()));
        let engine = Engine::start(&dir).expect("start");
        let mint = engine
            .run_blocking(engine.add_mint("https://testnut.cashudevkit.org"))
            .expect("add mint");
        let deposit = engine.run_blocking(engine.start_deposit(&mint.url, 3000)).unwrap();
        engine
            .run_blocking(engine.finish_deposit(&mint.url, &deposit.quote_id))
            .expect("deposit");

        let mut session = CopSession::open(&engine.handle(), uri, None, None);
        let incoming = session.incoming();
        let mut events = Vec::new();
        let pump = |session: &mut CopSession,
                    events: &mut Vec<CopEvent>,
                    until: &dyn Fn(&CopSession, &[CopEvent]) -> bool| {
            for _ in 0..200 {
                if until(session, events) {
                    return;
                }
                let next = engine.run_blocking(async {
                    tokio::time::timeout(std::time::Duration::from_millis(500), incoming.recv()).await
                });
                if let Ok(Ok(event)) = next {
                    session.handle(event);
                    events.extend(session.take_events());
                }
            }
            panic!("timed out; events so far: {events:?}");
        };

        pump(&mut session, &mut events, &|s, _| find(&s.tree(), "pay"));
        println!("page has a pay button; name = {:?}", session.name());
        session.act("add-espresso", "onPress");
        pump(&mut session, &mut events, &|s, _| {
            fn enabled(t: &[hypereffect::client::ResolvedTree]) -> bool {
                t.iter().any(|n| {
                    (n.id == "pay"
                        && n.attrs.get("disabled").and_then(|v| v.as_bool()) != Some(true))
                        || enabled(&n.children)
                })
            }
            enabled(&s.tree())
        });
        session.act("pay", "onPress");
        pump(&mut session, &mut events, &|_, events| {
            events.iter().any(|e| matches!(e, CopEvent::Invoice(_)))
        });
        let Some(CopEvent::Invoice(offer)) = events
            .iter()
            .find(|e| matches!(e, CopEvent::Invoice(_)))
            .cloned()
        else {
            unreachable!()
        };
        let offer = engine.run_blocking(engine.offer(offer.invoice)).expect("offer");
        println!("offer: {} sat, options {:?}", offer.amount_sat, offer.options);
        let paid = engine
            .run_blocking(engine.pay(&offer, &offer.options[0].mint_url, Some("demo".into()), None))
            .expect("pay");
        println!("paid {paid:?}");

        let txs = engine.run_blocking(engine.transactions(None)).unwrap();
        for tx in &txs {
            println!("{tx:?}");
        }
        assert!(txs.iter().any(|t| t.kind == TxKind::CassisSent && t.status == TxStatus::Done));

        // The server then offers a refund when asked.
        pump(&mut session, &mut events, &|s, _| find(&s.tree(), "refund"));
        session.act("refund", "onPress");
        pump(&mut session, &mut events, &|_, events| {
            events.iter().any(|e| matches!(e, CopEvent::InvoiceRequest(_)))
        });
        let Some(CopEvent::InvoiceRequest(request)) = events
            .iter()
            .find(|e| matches!(e, CopEvent::InvoiceRequest(_)))
            .cloned()
        else {
            unreachable!()
        };
        println!("refund offered: {request:?}");
        let mints = engine
            .run_blocking(engine.receivable_mints(&request.networks))
            .unwrap();
        let invoice = engine
            .run_blocking(engine.invoice_for_offer(
                &mints[0].0,
                request.amount_sat,
                request.memo.clone(),
                request.expires_in,
                Some("demo".into()),
            ))
            .expect("invoice");
        session.send_invoice(invoice);
        for _ in 0..30 {
            engine.run_blocking(engine.reconcile_cassis());
            let txs = engine.run_blocking(engine.transactions(None)).unwrap();
            if txs
                .iter()
                .any(|t| t.kind == TxKind::CassisReceived && t.status == TxStatus::Done)
            {
                println!("refund received");
                let _ = std::fs::remove_dir_all(dir);
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        panic!("the refund never arrived");
    }
}
