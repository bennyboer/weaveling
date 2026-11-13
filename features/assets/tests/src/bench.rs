use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use access::{HearsAccess, HearsOperators, InMemoryAccess, KeptAccess, Role};
use assets_catalog::{InMemoryAccountLimits, InMemoryAssetCatalog};
use assets_contents::InMemoryContents;
use assets_core::{
    AccountLimits, AssetCatalog, AssetsCore, Format, Held, Holders, ImageRefused, Images, Limits,
    Picture, Prepared, ProjectLink, Sorted, Usage,
};
use authorizing::{AllowingEverything, Authorizer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore, Version};
use transacting::{InMemoryTransactions, Transactions};

pub const THE_LOOM: &str = "project_loom";
pub const THE_WEFT: &str = "project_weft";
/// Prepares any upload as a JPEG of 1,280 pixels made into three widths, each variant naming its width, or refuses every upload; counting how often it was asked.
pub struct FakeImages {
    refusing: Option<ImageRefused>,
    asked: AtomicUsize,
}

impl FakeImages {
    pub fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

impl Images for FakeImages {
    fn prepared(&self, uploaded: &[u8]) -> Result<Prepared, ImageRefused> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        if let Some(refused) = &self.refusing {
            return Err(refused.clone());
        }

        Ok(Prepared {
            picture: Picture {
                format: Format::Jpeg,
                width: 1_280,
                height: 1_024,
                original_size: uploaded.len() as u64,
                variants: vec![384, 768, 1_280],
            },
            variants: [384, 768, 1_280]
                .into_iter()
                .map(|width| (width, format!("the {width} wide variant").into_bytes()))
                .collect(),
        })
    }
}

pub struct Bench {
    pub assets: AssetsCore,
    pub images: Arc<FakeImages>,
    pub contents: Arc<InMemoryContents>,
    pub catalog: Arc<InMemoryAssetCatalog>,
    account_limits: Arc<InMemoryAccountLimits>,
    transactions: Arc<InMemoryTransactions>,
}

pub fn ada() -> Agent {
    Agent::Account(AccountId::from("account_ada"))
}

pub fn bo() -> Agent {
    Agent::Account(AccountId::from("account_bo"))
}

pub fn grace() -> Agent {
    Agent::Account(AccountId::from("account_grace"))
}

/// Ada owning the loom and Bo reading it, as the members told the assets, who may do what asked of what the assets heard.
pub async fn a_bench() -> Bench {
    a_bench_limited(Limits::default()).await
}

pub async fn a_bench_limited(limits: Limits) -> Bench {
    bench_of(limits, None, true).await
}

pub async fn a_bench_refusing(refused: ImageRefused) -> Bench {
    bench_of(Limits::default(), Some(refused), true).await
}

/// Without accounts, where everyone may do everything.
pub async fn a_bench_without_accounts(limits: Limits) -> Bench {
    bench_of(limits, None, false).await
}

async fn bench_of(limits: Limits, refusing: Option<ImageRefused>, with_accounts: bool) -> Bench {
    let images = Arc::new(FakeImages {
        refusing,
        asked: AtomicUsize::new(0),
    });
    let contents = Arc::new(InMemoryContents::new());
    let catalog = Arc::new(InMemoryAssetCatalog::new());
    let transactions = Arc::new(InMemoryTransactions::new());
    let access = KeptAccess::new(Arc::new(InMemoryAccess::new()));
    let account_limits = Arc::new(InMemoryAccountLimits::new());
    let authorizer: Arc<dyn Authorizer> = if with_accounts {
        Arc::new(access.clone())
    } else {
        Arc::new(AllowingEverything)
    };

    let bench = Bench {
        assets: AssetsCore::new(
            Arc::new(InMemoryEventStore::new()),
            transactions.clone(),
            catalog.clone(),
            access,
            account_limits.clone(),
            contents.clone(),
            images.clone(),
            authorizer,
            Arc::new(FixedClock::new(clock::at(1_700_000_000))),
        )
        .limited(limits),
        images,
        contents,
        catalog,
        account_limits,
        transactions,
    };
    for (account, version, role) in [
        ("account_ada", 1, Role::Owner),
        ("account_bo", 2, Role::Reader),
    ] {
        bench
            .assets
            .remember_role(
                None,
                THE_LOOM,
                &AccountId::from(account),
                Version::of(version),
                Some(role),
            )
            .await
            .expect("remembering should succeed");
    }

    bench
}

impl Bench {
    /// Tells the assets Ada runs the instance, as the accounts would.
    pub async fn ada_runs_the_instance(&self) {
        self.assets
            .remember_operator(None, &AccountId::from("account_ada"), Version::of(3), true)
            .await
            .expect("remembering should succeed");
    }

    /// Gives Ada a limit of her own, as the operator would, or takes hers back.
    pub async fn adas_own_limit(&self, most: Option<u64>) {
        let ada = AccountId::from("account_ada");
        let mut transaction = self
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");
        match most {
            Some(most) => self.account_limits.give(&mut transaction, &ada, most).await,
            None => self.account_limits.take_back(&mut transaction, &ada).await,
        }
        .expect("the limits should be reached");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");
    }

    /// The accounts or projects holding anything, by id, with their names as far as the assets heard; those alone found by the term, where one is given.
    pub async fn held(&self, holders: Holders, matching: Option<&str>) -> Vec<Held> {
        let mut transaction = self
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");
        let held = self
            .catalog
            .held_by(&mut transaction, holders, Sorted::ById, None, matching, 50)
            .await
            .expect("asking should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        held
    }

    /// What the installation, Ada and the loom hold.
    pub async fn usage(&self) -> Usage {
        let mut transaction = self
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");
        let usage = self
            .catalog
            .usage_within(
                &mut transaction,
                Some(&AccountId::from("account_ada")),
                Some(&ProjectLink::from(THE_LOOM)),
            )
            .await
            .expect("asking should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        usage
    }
}
