use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use messaging::Message;
use outbox::{Outbox, SqliteOutbox};
use passages_core::{IdeaLink, PassageStore, StoreError};
use sqlx::{Row, SqlitePool};
use test_harness::SqliteFixture;

use crate::sqlite::{SqlitePassageStore, migrations};
use crate::suite::{Heard, Workbench, a_paragraph, a_passage, an_id, at, message_for};

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqlitePassageStore,
    heard: Arc<Heard>,
}

struct Compacting(OnSqlite);

fn a_clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(at(2_000)))
}

fn enqueuing(store: SqlitePassageStore) -> SqlitePassageStore {
    store.enqueuing(a_clock(), message_for)
}

async fn relayed(pool: &SqlitePool, heard: &Arc<Heard>) -> Vec<Message> {
    SqliteOutbox::new(pool.clone(), heard.clone(), a_clock())
        .deliver(100)
        .await
        .expect("the outbox should deliver");

    heard.messages()
}

async fn a_migrated_schema() -> (SqliteFixture, SqlitePool) {
    let fixture = SqliteFixture::setup();
    let pool = fixture.create_database("passages").await;
    outbox::sqlite::migrations()
        .run(&pool)
        .await
        .expect("the outbox should lay down in an empty file");
    migrations()
        .run(&pool)
        .await
        .expect("the passage schema should lay down in an empty file");

    (fixture, pool)
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqlitePassageStore;

    async fn setup() -> Self {
        let (fixture, pool) = a_migrated_schema().await;

        Self {
            fixture,
            store: enqueuing(SqlitePassageStore::new(pool.clone())),
            pool,
            heard: Arc::new(Heard::default()),
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
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
    type Store = SqlitePassageStore;

    async fn setup() -> Self {
        let (fixture, pool) = a_migrated_schema().await;

        Self(OnSqlite {
            fixture,
            store: enqueuing(SqlitePassageStore::compacting_after(pool.clone(), 1)),
            pool,
            heard: Arc::new(Heard::default()),
        })
    }

    fn store(&self) -> &Self::Store {
        &self.0.store
    }

    async fn enqueued(&self) -> Vec<Message> {
        relayed(&self.0.pool, &self.0.heard).await
    }

    async fn cleanup(self) {
        self.0.cleanup().await;
    }
}

impl OnSqlite {
    async fn rows(&self) -> Vec<bool> {
        sqlx::query_scalar("SELECT is_snapshot FROM passage_updates ORDER BY seq")
            .fetch_all(&self.pool)
            .await
            .expect("reading the parts should succeed")
    }

    async fn passages(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM passages")
            .fetch_one(&self.pool)
            .await
            .expect("counting passages should succeed")
    }
}

mod appending {
    crate::suite::conformance_tests!(super::OnSqlite);
}

mod compacting {
    crate::suite::conformance_tests!(super::Compacting);
}

#[tokio::test]
async fn an_update_is_one_row_and_never_rewrites_the_passage() {
    let bench = OnSqlite::setup().await;
    let id = an_id(1_000);
    bench
        .store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    bench
        .store
        .apply(id, &a_paragraph("Then it began."))
        .await
        .expect("apply should succeed");

    assert_eq!(
        bench.rows().await,
        vec![true, false],
        "a snapshot from create, then one appended update — the passage itself is never rewritten"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_tail_longer_than_we_keep_is_collapsed_into_a_snapshot() {
    let bench = OnSqlite::setup().await;
    let store = SqlitePassageStore::compacting_after(bench.pool.clone(), 3);
    let id = an_id(1_000);
    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    for nth in 0..5 {
        store
            .apply(id, &a_paragraph(&format!("Line {nth}.")))
            .await
            .expect("apply should succeed");
    }

    let parts = bench.rows().await;

    assert!(
        parts.len() < 6,
        "six writes should not have left six rows: {parts:?}"
    );
    assert_eq!(
        parts.first(),
        Some(&true),
        "what remains must begin with a snapshot: {parts:?}"
    );

    let found = store.load(id).await.expect("load should succeed");
    assert!(
        found.text().contains("The loom stood silent.") && found.text().contains("Line 4."),
        "compaction must not lose prose, got {:?}",
        found.text()
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn compacting_a_short_tail_does_nothing() {
    let bench = OnSqlite::setup().await;
    let id = an_id(1_000);
    bench
        .store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    bench
        .store
        .apply(id, &a_paragraph("Then it began."))
        .await
        .expect("apply should succeed");

    let collapsed = bench
        .store
        .compact(id)
        .await
        .expect("compacting should succeed");

    assert!(!collapsed, "two rows are not worth collapsing");
    assert_eq!(bench.rows().await.len(), 2);

    bench.cleanup().await;
}

#[tokio::test]
async fn compacting_a_passage_that_was_never_created_is_not_found() {
    let bench = OnSqlite::setup().await;
    let never_written = an_id(1_000);

    let refused = bench.store.compact(never_written).await;

    assert!(
        matches!(refused, Err(StoreError::NotFound(missing)) if missing == never_written),
        "got {refused:?}"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn an_update_nothing_can_read_writes_no_row() {
    let bench = OnSqlite::setup().await;
    let id = an_id(1_000);
    bench
        .store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    let refused = bench.store.apply(id, &[255, 255, 255, 255]).await;

    assert!(
        matches!(refused, Err(StoreError::Unusable(_))),
        "got {refused:?}"
    );
    assert_eq!(
        bench.rows().await,
        vec![true],
        "garbage must not reach the table, or every load would fail forever after"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn an_update_for_a_passage_that_does_not_exist_writes_no_row() {
    let bench = OnSqlite::setup().await;

    let refused = bench
        .store
        .apply(an_id(1_000), &a_paragraph("Out of nowhere."))
        .await;

    assert!(
        matches!(refused, Err(StoreError::NotFound(_))),
        "got {refused:?}"
    );
    assert!(bench.rows().await.is_empty());

    bench.cleanup().await;
}

#[tokio::test]
async fn deleting_a_passage_takes_its_updates_with_it() {
    let bench = OnSqlite::setup().await;
    let id = an_id(1_000);
    bench
        .store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    bench
        .store
        .apply(id, &a_paragraph("Then it began."))
        .await
        .expect("apply should succeed");

    bench.store.delete(id).await.expect("delete should succeed");

    assert_eq!(bench.passages().await, 0);
    assert!(
        bench.rows().await.is_empty(),
        "the parts are the passage — leaving them would leave a passage that load cannot find"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_passage_survives_being_reloaded_by_a_store_that_never_saw_the_writes() {
    let bench = OnSqlite::setup().await;
    let id = an_id(1_000);
    bench
        .store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    bench
        .store
        .apply(id, &a_paragraph("Then it began."))
        .await
        .expect("apply should succeed");

    let elsewhere = SqlitePassageStore::new(bench.pool.clone());
    let found = elsewhere.load(id).await.expect("load should succeed");

    assert!(
        found.text().contains("The loom stood silent.") && found.text().contains("Then it began."),
        "prose has to come back out of the rows, not out of whoever wrote them, got {:?}",
        found.text()
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn walking_a_project_comes_out_of_the_index() {
    let bench = OnSqlite::setup().await;

    let plan: Vec<String> = sqlx::query(
        "EXPLAIN QUERY PLAN
         SELECT passage
         FROM passages
         WHERE project = ?1 AND passage > ?2
         ORDER BY passage
         LIMIT ?3",
    )
    .bind("project_3")
    .bind("passage_0000000000000000100")
    .bind(100_i64)
    .fetch_all(&bench.pool)
    .await
    .expect("explaining should succeed")
    .iter()
    .map(|step| step.get::<String, _>("detail"))
    .collect();
    let plan = plan.join(
        "
",
    );

    assert!(
        plan.contains("USING COVERING INDEX passages_by_project")
            || plan.contains("USING INDEX passages_by_project"),
        "deleting a project sweeps its prose one batch at a time, so the batch must come out of the index: {plan}"
    );
    assert!(
        !plan.contains("SCAN passages") && !plan.contains("TEMP B-TREE"),
        "a scan or a sort here is silent; the sweep keeps working and only gets slower: {plan}"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn deleting_a_passage_takes_its_links_with_it() {
    let bench = OnSqlite::setup().await;
    let id = an_id(1_000);
    bench
        .store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    bench
        .store
        .link(id, &IdeaLink::from("idea_1"))
        .await
        .expect("link should succeed");

    bench.store.delete(id).await.expect("delete should succeed");

    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM passage_ideas")
        .fetch_one(&bench.pool)
        .await
        .expect("counting should succeed");
    assert_eq!(
        links, 0,
        "a link outliving its passage points at nothing, and nothing sweeps it later"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_tail_exactly_as_long_as_we_keep_is_left_as_it_is() {
    let bench = OnSqlite::setup().await;
    let store = SqlitePassageStore::compacting_after(bench.pool.clone(), 3);
    let id = an_id(1_000);
    store
        .create(&a_passage(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    for nth in 0..3 {
        store
            .apply(id, &a_paragraph(&format!("Line {nth}.")))
            .await
            .expect("apply should succeed");
    }

    assert_eq!(
        bench.rows().await,
        vec![true, false, false, false],
        "keeping three updates means compacting on the fourth, not the third"
    );

    bench.cleanup().await;
}
