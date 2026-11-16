use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use access::{HearsAccess, HearsOperators, InMemoryAccess, KeptAccess, Role};
use assets_catalog::{InMemoryAccountLimits, InMemoryAssetCatalog, InMemoryAssetUses};
use assets_contents::InMemoryContents;
use assets_core::{
    AccountLimits, AssetCatalog, AssetId, AssetsCore, Contents, ContentsError, Format, Held,
    Holders, ImageInfo, ImageRefused, Images, Limits, Piece, Prepared, ProjectLink, Sorted, Usage,
};
use async_trait::async_trait;
use authorizing::{AllowingEverything, Authorizer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore, Version};
use messaging::{FakePublisher, Message};
use outbox::{InMemoryOutbox, Outbox};
use transacting::{InMemoryTransactions, Transactions};

pub const THE_LOOM: &str = "project_loom";
pub const THE_WEFT: &str = "project_weft";
/// Prepares any upload as a JPEG of 1,280 pixels, made into three widths, each variant naming its width.
/// Or it refuses every upload.
/// It counts how often it was asked.
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
            image: ImageInfo {
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

/// Stores the pieces in memory, and counts how often a piece was stored, even if it was removed again.
pub struct WatchedContents {
    held: InMemoryContents,
    kept: AtomicUsize,
}

impl WatchedContents {
    /// Returns how many pieces are stored now, across all assets.
    pub fn pieces(&self) -> usize {
        self.held.pieces()
    }

    /// Returns how often a piece was stored, counting those removed since.
    pub fn times_kept(&self) -> usize {
        self.kept.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Contents for WatchedContents {
    async fn keep(&self, asset: &AssetId, piece: Piece, bytes: &[u8]) -> Result<(), ContentsError> {
        self.kept.fetch_add(1, Ordering::SeqCst);
        self.held.keep(asset, piece, bytes).await
    }

    async fn read(&self, asset: &AssetId, piece: Piece) -> Result<Option<Vec<u8>>, ContentsError> {
        self.held.read(asset, piece).await
    }

    async fn forget(&self, asset: &AssetId) -> Result<(), ContentsError> {
        self.held.forget(asset).await
    }

    async fn stored(
        &self,
        after: Option<&AssetId>,
        at_most: u64,
    ) -> Result<Vec<AssetId>, ContentsError> {
        self.held.stored(after, at_most).await
    }
}

pub struct Wired {
    pub assets: AssetsCore,
    pub images: Arc<FakeImages>,
    pub contents: Arc<WatchedContents>,
    pub catalog: Arc<InMemoryAssetCatalog>,
    pub clock: Arc<FixedClock>,
    pub outbox: Arc<InMemoryOutbox>,
    published: Arc<FakePublisher>,
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

/// A workbench where Ada owns the loom and Bo reads it, as the members feature reported it.
/// Who may do what is decided by what the assets feature heard.
pub async fn a_workbench() -> Wired {
    a_workbench_limited(Limits::default()).await
}

pub async fn a_workbench_limited(limits: Limits) -> Wired {
    a_workbench_of(limits, None, Instance::WithAccounts).await
}

pub async fn a_workbench_refusing(refused: ImageRefused) -> Wired {
    a_workbench_of(Limits::default(), Some(refused), Instance::WithAccounts).await
}

/// A workbench without accounts, where everyone may do everything.
pub async fn a_workbench_without_accounts(limits: Limits) -> Wired {
    a_workbench_of(limits, None, Instance::WithoutAccounts).await
}

/// Whether the instance a workbench stands for has accounts.
/// With accounts, who may do what is decided by what the assets feature heard.
/// Without, everyone may do everything.
enum Instance {
    WithAccounts,
    WithoutAccounts,
}

async fn a_workbench_of(
    limits: Limits,
    refusing: Option<ImageRefused>,
    instance: Instance,
) -> Wired {
    let images = Arc::new(FakeImages {
        refusing,
        asked: AtomicUsize::new(0),
    });
    let contents = Arc::new(WatchedContents {
        held: InMemoryContents::new(),
        kept: AtomicUsize::new(0),
    });
    let catalog = Arc::new(InMemoryAssetCatalog::new());
    let transactions = Arc::new(InMemoryTransactions::new());
    let access = KeptAccess::new(Arc::new(InMemoryAccess::new()));
    let account_limits = Arc::new(InMemoryAccountLimits::new());
    let uses = Arc::new(InMemoryAssetUses::new());
    let authorizer: Arc<dyn Authorizer> = match instance {
        Instance::WithAccounts => Arc::new(access.clone()),
        Instance::WithoutAccounts => Arc::new(AllowingEverything),
    };

    let clock = Arc::new(FixedClock::new(clock::at(1_700_000_000)));
    let published = Arc::new(FakePublisher::default());
    let wired = Wired {
        assets: AssetsCore::new(
            Arc::new(InMemoryEventStore::new()),
            transactions.clone(),
            catalog.clone(),
            access,
            account_limits.clone(),
            contents.clone(),
            images.clone(),
            authorizer,
            uses,
            clock.clone(),
        )
        .limited(limits),
        images,
        contents,
        catalog,
        clock: clock.clone(),
        outbox: Arc::new(InMemoryOutbox::new(published.clone(), clock)),
        published,
        account_limits,
        transactions,
    };
    for (account, version, role) in [
        ("account_ada", 1, Role::Owner),
        ("account_bo", 2, Role::Reader),
    ] {
        wired
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

    wired
}

impl Wired {
    /// Publishes the follow-ups the listeners queued, and returns every follow-up published so far.
    pub async fn follow_ups(&self) -> Vec<Message> {
        self.outbox
            .deliver(100)
            .await
            .expect("delivering should succeed");

        self.published.heard()
    }

    /// Makes Ada an owner of the weft too, as the members feature would report it.
    pub async fn ada_owns_the_weft(&self) {
        self.assets
            .remember_role(
                None,
                THE_WEFT,
                &AccountId::from("account_ada"),
                Version::of(4),
                Some(Role::Owner),
            )
            .await
            .expect("remembering should succeed");
    }

    /// Tells the assets feature that Ada runs the instance, as the accounts feature would.
    pub async fn ada_runs_the_instance(&self) {
        self.assets
            .remember_operator(None, &AccountId::from("account_ada"), Version::of(3), true)
            .await
            .expect("remembering should succeed");
    }

    /// Gives Ada her own limit, as an operator would.
    /// With none, her own limit is removed.
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

    /// Returns the accounts or projects that store anything, by id, with their names as far as the assets feature heard them.
    /// With a search term, only those matching the term are returned.
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

    /// Returns how many bytes the installation, Ada and the loom store.
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
