use std::sync::Arc;

use async_trait::async_trait;
use invitations_core::{CatalogError, InvitationCatalog, InvitationId, TokenKeys};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use transacting::{SqliteTransactions, Transactions};

use crate::sqlite;
use crate::sqlite::SqliteInvitationCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    transactions: SqliteTransactions,
    store: SqliteInvitationCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteInvitationCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("invitations").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
            store: SqliteInvitationCatalog::new(pool.clone()),
            pool,
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::conformance_tests!(OnSqlite);

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_invitation_standing() {
    // given: the list on sqlite
    let bench = OnSqlite::setup().await;
    let catalog = Arc::new(SqliteInvitationCatalog::new(bench.pool.clone()));
    let transactions = Arc::new(SqliteTransactions::new(bench.pool.clone()));

    // when: invitations are remembered and forgotten at once, again and again
    for nth in 0..40 {
        let summary = suite::a_summary(
            InvitationId::generate(clock::at(1_000 + nth)),
            "ada@example.com",
        );
        let (remembering, forgetting) = (catalog.clone(), catalog.clone());
        let (remembered_in, forgotten_in) = (transactions.clone(), transactions.clone());
        let id = summary.id;
        let (remembered, forgotten) = tokio::join!(
            tokio::spawn(async move {
                let mut transaction = remembered_in
                    .begin()
                    .await
                    .expect("beginning should succeed");
                remembering.remember(&mut transaction, &summary).await?;
                transaction
                    .finish()
                    .await
                    .expect("finishing should succeed");
                Ok::<_, CatalogError>(())
            }),
            tokio::spawn(async move {
                let mut transaction = forgotten_in
                    .begin()
                    .await
                    .expect("beginning should succeed");
                forgetting
                    .forget(&mut transaction, &id, clock::at(5_000))
                    .await?;
                transaction
                    .finish()
                    .await
                    .expect("finishing should succeed");
                Ok::<_, CatalogError>(())
            }),
        );
        remembered
            .expect("remembering should not panic")
            .expect("remembering should succeed");
        forgotten
            .expect("forgetting should not panic")
            .expect("forgetting should succeed");
    }

    // then: none of them stands
    assert!(
        catalog
            .listed_after(None, 100)
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a remember racing a forget must never bring the forgotten invitation back"
    );
    bench.cleanup().await;
}

#[tokio::test]
async fn the_token_key_is_drawn_once_and_kept_however_many_ask_at_once() {
    // given: the token keys on sqlite
    let bench = OnSqlite::setup().await;
    let keys = Arc::new(sqlite::SqliteTokenKeys::new(bench.pool.clone()));

    // when: many ask for the key at once
    let asked: Vec<_> = futures_util::future::join_all((0..8).map(|_| {
        let keys = keys.clone();
        async move { keys.key().await.expect("a key is at hand") }
    }))
    .await;

    // then: all are told the one key
    assert!(
        asked.iter().all(|key| *key == asked[0]),
        "every invitation issued and mailed is derived under the one key"
    );
    // and: a restart finds it
    assert_eq!(
        sqlite::SqliteTokenKeys::new(bench.pool.clone())
            .key()
            .await
            .expect("a key is at hand"),
        asked[0],
        "a restart finds the key it drew"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_key_once_drawn_is_read_while_another_writer_holds_the_file() {
    // given: a key drawn
    let bench = OnSqlite::setup().await;
    let keys = sqlite::SqliteTokenKeys::new(bench.pool.clone());
    let drawn = keys.key().await.expect("a key is at hand");
    // and: another writer holding the file
    let writing = SqliteTransactions::new(bench.pool.clone())
        .begin()
        .await
        .expect("a writer begins");

    // when: the key is asked for
    let read = tokio::time::timeout(std::time::Duration::from_secs(2), keys.key())
        .await
        .expect("a hearing's own writer must not keep its mail from the key")
        .expect("a key is at hand");

    // then: it is the key drawn
    assert_eq!(read, drawn);
    drop(writing);
    bench.cleanup().await;
}
