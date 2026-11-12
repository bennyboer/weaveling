use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use access::{InMemoryAccess, KeptAccess};
use assets_catalog::{InMemoryAccountLimits, InMemoryAssetCatalog};
use assets_contents::InMemoryContents;
use assets_core::{
    AccountLimits, AssetCatalog, AssetsCore, Format, ImageRefused, Images, Limits, Picture,
    Prepared, ProjectLink, Usage,
};
use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use transacting::{InMemoryTransactions, Transactions};

pub const THE_LOOM: &str = "project_loom";
pub const THE_WEFT: &str = "project_weft";
pub const OWNER: [Act; 3] = [Act::Read, Act::Write, Act::Manage];
pub const READER: [Act; 1] = [Act::Read];

/// Who may do what in which project, by account.
#[derive(Default)]
pub struct Roles(Mutex<HashMap<(String, String), Vec<Act>>>);

impl Roles {
    pub fn give(&self, account: &str, project: &str, acts: &[Act]) {
        self.0
            .lock()
            .expect("never poisoned")
            .insert((account.to_owned(), project.to_owned()), acts.to_vec());
    }
}

#[async_trait]
impl Authorizer for Roles {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        let Agent::Account(account) = agent else {
            return Ok(true);
        };

        Ok(self
            .0
            .lock()
            .expect("never poisoned")
            .get(&(account.to_string(), project.to_owned()))
            .is_some_and(|acts| acts.contains(&act)))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }
}

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

pub fn a_bench() -> Bench {
    a_bench_limited(Limits::default())
}

pub fn a_bench_limited(limits: Limits) -> Bench {
    bench_of(limits, None)
}

pub fn a_bench_refusing(refused: ImageRefused) -> Bench {
    bench_of(Limits::default(), Some(refused))
}

fn bench_of(limits: Limits, refusing: Option<ImageRefused>) -> Bench {
    let roles = Arc::new(Roles::default());
    roles.give("account_ada", THE_LOOM, &OWNER);
    roles.give("account_bo", THE_LOOM, &READER);
    let images = Arc::new(FakeImages {
        refusing,
        asked: AtomicUsize::new(0),
    });
    let contents = Arc::new(InMemoryContents::new());
    let catalog = Arc::new(InMemoryAssetCatalog::new());
    let transactions = Arc::new(InMemoryTransactions::new());
    let access = KeptAccess::new(Arc::new(InMemoryAccess::new()));
    let account_limits = Arc::new(InMemoryAccountLimits::new());

    Bench {
        assets: AssetsCore::new(
            Arc::new(InMemoryEventStore::new()),
            transactions.clone(),
            catalog.clone(),
            access,
            account_limits.clone(),
            contents.clone(),
            images.clone(),
            roles.clone(),
            Arc::new(FixedClock::new(clock::at(1_700_000_000))),
        )
        .limited(limits),
        images,
        contents,
        catalog,
        account_limits,
        transactions,
    }
}

impl Bench {
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
