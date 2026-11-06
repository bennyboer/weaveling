use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{FakePublisher, Message};
use outbox::{Outbox, PostgresOutbox};
use scenes_catalog::PostgresSceneCatalog;
use scenes_core::{Scene, SceneCatalog, SceneId, SceneStore, StoreError};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use tokio::task::JoinSet;
use transacting::{Failure, PostgresTransactions, Transactions, Tried};

use crate::postgres;
use crate::postgres::PostgresSceneStore;
use crate::suite;
use crate::suite::{Tables, Workbench};

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    store: PostgresSceneStore,
    heard: Arc<FakePublisher>,
}

struct Compacting(OnPostgres);

fn a_clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(clock::at(2_000)))
}

fn enqueuing(store: PostgresSceneStore) -> PostgresSceneStore {
    store.enqueuing(a_clock(), suite::message_for)
}

async fn relayed(pool: &PgPool, heard: &Arc<FakePublisher>) -> Vec<Message> {
    PostgresOutbox::new(pool.clone(), heard.clone(), a_clock())
        .deliver(100)
        .await
        .expect("the outbox should deliver");

    heard.heard()
}

async fn a_migrated_schema() -> (PostgresFixture, PgPool) {
    let fixture = PostgresFixture::setup().await;
    let pool = fixture.create_schema("scenes").await;
    outbox::postgres::migrations()
        .run(&pool)
        .await
        .expect("the outbox should lay down in an empty namespace");
    postgres::migrations()
        .run(&pool)
        .await
        .expect("the scene schema should lay down in an empty namespace");

    (fixture, pool)
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresSceneStore;

    async fn setup() -> Self {
        let (fixture, pool) = a_migrated_schema().await;

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            store: enqueuing(PostgresSceneStore::new(pool.clone())),
            pool,
            heard: Arc::new(FakePublisher::default()),
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn enqueued(&self) -> Vec<Message> {
        relayed(&self.pool, &self.heard).await
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

#[async_trait]
impl Workbench for Compacting {
    type Store = PostgresSceneStore;

    async fn setup() -> Self {
        let (fixture, pool) = a_migrated_schema().await;

        Self(OnPostgres {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            store: enqueuing(PostgresSceneStore::compacting_after(pool.clone(), 1)),
            pool,
            heard: Arc::new(FakePublisher::default()),
        })
    }

    fn store(&self) -> &Self::Store {
        &self.0.store
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.0.transactions
    }

    async fn enqueued(&self) -> Vec<Message> {
        relayed(&self.0.pool, &self.0.heard).await
    }

    async fn cleanup(self) {
        self.0.cleanup().await;
    }
}

#[async_trait]
impl Tables for OnPostgres {
    fn on_the_same_tables(&self, compacting_after: i64) -> PostgresSceneStore {
        PostgresSceneStore::compacting_after(self.pool.clone(), compacting_after)
    }

    async fn compact(&self, id: SceneId) -> Result<bool, StoreError> {
        self.store.compact(id).await
    }

    async fn rows(&self) -> Vec<bool> {
        sqlx::query_scalar("SELECT is_snapshot FROM scene_updates ORDER BY seq")
            .fetch_all(&self.pool)
            .await
            .expect("reading the parts should succeed")
    }

    async fn count(&self, query: &'static str) -> i64 {
        sqlx::query_scalar(query)
            .fetch_one(&self.pool)
            .await
            .expect("counting should succeed")
    }
}

mod appending {
    crate::suite::conformance_tests!(super::OnPostgres);
}

mod tables {
    crate::suite::table_tests!(super::OnPostgres);
}

mod compacting {
    crate::suite::conformance_tests!(super::Compacting);
}

#[tokio::test]
async fn walking_a_project_comes_out_of_the_index() {
    // given: many scenes across several projects
    let bench = OnPostgres::setup().await;
    let mut written = Vec::new();
    for nth in 1..=2_000 {
        written.push(format!("scene_{nth:0>19}"));
    }

    sqlx::query(
        "INSERT INTO scenes (scene, project, title)
         SELECT held, 'project_' || (ordinality % 8), ''
         FROM unnest($1::text[]) WITH ORDINALITY AS held",
    )
    .bind(&written)
    .execute(&bench.pool)
    .await
    .expect("seeding should succeed");

    sqlx::query("ANALYZE scenes")
        .execute(&bench.pool)
        .await
        .expect("analysing should succeed");

    // when: walking one project after a cursor is planned
    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN SELECT scene
         FROM scenes
         WHERE project = $1 AND scene > $2
         ORDER BY scene
         LIMIT $3",
    )
    .bind("project_3")
    .bind("scene_0000000000000000100")
    .bind(100_i64)
    .fetch_all(&bench.pool)
    .await
    .expect("explaining should succeed");
    let plan = plan.join("\n");

    // then: the walk comes out of the project's index
    assert!(
        plan.contains("scenes_by_project"),
        "deleting a project sweeps its prose one batch at a time, so the batch must come out \
         of the index rather than a scan that grows with every book ever written: {plan}"
    );
    // and: never from a scan
    assert!(
        !plan.contains("Seq Scan"),
        "a plan change here is silent — the sweep keeps working and only gets slower: {plan}"
    );
    // and: reads no row it throws away
    assert!(
        !plan.contains("Filter:"),
        "both halves belong in the index condition — a filter means rows are read and then \
         thrown away, which is the cost the cursor exists to avoid: {plan}"
    );

    bench.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_edit_written_while_its_scene_is_compacted_is_never_lost() {
    // given: a stored scene in a store that compacts after two updates
    let (fixture, pool) = a_migrated_schema().await;
    let transactions = Arc::new(PostgresTransactions::new(pool.clone()));
    let store = Arc::new(PostgresSceneStore::compacting_after(pool, 2));
    let id = suite::an_id(1_000);
    let mut transaction = transactions
        .begin()
        .await
        .expect("beginning should succeed");
    store
        .create(&mut transaction, &Scene::empty(id, suite::a_project()))
        .await
        .expect("creating should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // when: it is edited and compacted at once, again and again
    let mut writing: JoinSet<Result<(), StoreError>> = JoinSet::new();
    let (editing, edits_in) = (store.clone(), transactions.clone());
    writing.spawn(async move {
        for nth in 0..200 {
            transacting::retrying!(&*edits_in, |transaction| {
                editing
                    .apply(transaction, id, &suite::a_paragraph(&format!("[{nth}]")))
                    .await
            })?;
        }
        Ok(())
    });
    let compacting = store.clone();
    writing.spawn(async move {
        for _ in 0..50 {
            if let Err(why) = compacting.compact(id).await
                && !matches!(why, StoreError::Backend(_))
            {
                return Err(why);
            }
        }
        Ok(())
    });
    while let Some(written) = writing.join_next().await {
        written
            .expect("writing should not panic")
            .expect("writing should succeed");
    }

    // then: every edit is in its prose
    let text = store.load(id).await.expect("loading should succeed").text();
    let lost: Vec<usize> = (0..200)
        .filter(|nth| !text.contains(&format!("[{nth}]")))
        .collect();
    assert!(
        lost.is_empty(),
        "edits written while the scene was compacted were lost: {lost:?}"
    );
    fixture.cleanup().await;
}

#[tokio::test]
async fn a_create_that_found_no_tombstone_loses_to_one_laid_and_swept_before_it_commits() {
    // given: a create in a project that found no tombstone, not yet committed
    let (fixture, pool) = a_migrated_schema().await;
    scenes_catalog::postgres::migrations()
        .run(&pool)
        .await
        .expect("the catalog should lay down beside the store");
    let transactions = PostgresTransactions::new(pool.clone());
    let store = PostgresSceneStore::new(pool.clone());
    let catalog = PostgresSceneCatalog::new(pool);
    let project = suite::a_project();
    let mut creating = transactions
        .begin()
        .await
        .expect("beginning should succeed");
    assert!(
        !catalog
            .has_project_tombstone_within(&mut creating, &project)
            .await
            .expect("asking should succeed")
    );
    store
        .create(
            &mut creating,
            &Scene::empty(suite::an_id(1_000), project.clone()),
        )
        .await
        .expect("creating should succeed");

    // when: the project's tombstone is laid
    transacting::retrying!(&transactions, |burying| {
        catalog
            .lay_project_tombstone(burying, &project, OffsetDateTime::UNIX_EPOCH)
            .await
            .map_err(Tried::Refused)
    })
    .expect("laying a tombstone should succeed");
    // and: the project is swept
    let swept = store
        .in_project(&project, None, 10)
        .await
        .expect("the sweep should read the scenes");

    // then: the sweep finds no scene
    assert!(
        swept.is_empty(),
        "the sweep cannot see a scene not committed yet"
    );
    // and: the create loses the race
    assert!(
        matches!(creating.finish().await, Err(Failure::LostRace)),
        "the create read no tombstone and the sweep read no scene, so the create, between the two, \
         loses and is tried again, then finding the tombstone"
    );
    fixture.cleanup().await;
}
