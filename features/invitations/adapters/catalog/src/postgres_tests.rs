use std::sync::Arc;

use async_trait::async_trait;
use eventsourcing::Version;
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, ProjectLink, ProjectName, TokenKeys,
};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{PostgresTransactions, Transaction, Transactions, Tried};

use crate::postgres;
use crate::postgres::PostgresInvitationCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    store: PostgresInvitationCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresInvitationCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("invitations").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            store: PostgresInvitationCatalog::new(pool.clone()),
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

crate::conformance_tests!(OnPostgres);

#[tokio::test]
async fn the_id_column_sorts_bytewise_whatever_the_database_locale_is() {
    let bench = OnPostgres::setup().await;

    let collation: Option<String> = sqlx::query_scalar(
        "SELECT collation_name::text
         FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = 'invitation_summaries' AND column_name = 'invitation'",
    )
    .bind(bench.fixture.schema_of("invitations"))
    .fetch_one(&bench.pool)
    .await
    .expect("reading the list should succeed");

    assert_eq!(
        collation.as_deref(),
        Some("C"),
        "the id orders the listing and pages it, so it must sort bytewise"
    );

    bench.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_invitation_standing() {
    let bench = OnPostgres::setup().await;
    let catalog = Arc::new(PostgresInvitationCatalog::new(bench.pool.clone()));
    let transactions = Arc::new(PostgresTransactions::new(bench.pool.clone()));

    for nth in 0..40 {
        let summary = suite::a_summary(
            InvitationId::generate(suite::at(1_000 + nth)),
            "ada@example.com",
        );
        let (remembering, forgetting) = (catalog.clone(), catalog.clone());
        let (remembered_in, forgotten_in) = (transactions.clone(), transactions.clone());
        let id = summary.id;
        let (remembered, forgotten) = tokio::join!(
            tokio::spawn(async move {
                transacting::retrying!(&*remembered_in, |transaction| {
                    remembering
                        .remember(transaction, &summary)
                        .await
                        .map_err(Tried::Refused)?;
                    Ok::<_, Tried<CatalogError>>(())
                })
            }),
            tokio::spawn(async move {
                transacting::retrying!(&*forgotten_in, |transaction| {
                    forgetting
                        .forget(transaction, &id, suite::at(5_000))
                        .await
                        .map_err(Tried::Refused)?;
                    Ok::<_, Tried<CatalogError>>(())
                })
            }),
        );
        remembered
            .expect("remembering should not panic")
            .expect("remembering should succeed");
        forgotten
            .expect("forgetting should not panic")
            .expect("forgetting should succeed");
    }

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
    let bench = OnPostgres::setup().await;
    let keys = Arc::new(postgres::PostgresTokenKeys::new(bench.pool.clone()));

    let asked: Vec<_> = futures_util::future::join_all((0..8).map(|_| {
        let keys = keys.clone();
        async move { keys.key().await.expect("a key is at hand") }
    }))
    .await;

    assert!(
        asked.iter().all(|key| *key == asked[0]),
        "every invitation issued and mailed is derived under the one key"
    );
    assert_eq!(
        postgres::PostgresTokenKeys::new(bench.pool.clone())
            .key()
            .await
            .expect("a key is at hand"),
        asked[0],
        "a restart finds the key it drew"
    );

    bench.cleanup().await;
}

async fn buried_while(
    bench: OnPostgres,
    project: &ProjectLink,
    remembering: Transaction,
) -> OnPostgres {
    tokio::join!(
        async {
            transacting::retrying!(&bench.transactions, |burying| {
                bench
                    .store
                    .lay_project_tombstone(burying, project, OffsetDateTime::UNIX_EPOCH)
                    .await
                    .map_err(Tried::Refused)
            })
            .expect("laying a tombstone should succeed");
        },
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            remembering
                .finish()
                .await
                .expect("committing should succeed");
        }
    );

    bench
}

#[tokio::test]
async fn a_project_s_tombstone_waits_for_its_name_under_way() {
    let bench = OnPostgres::setup().await;
    let project = ProjectLink::from("project_loom");
    let mut naming = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store
        .remember_project_name(
            &mut naming,
            &ProjectName {
                project: project.clone(),
                version: Version::of(1),
                name: "The Loom".to_owned(),
            },
        )
        .await
        .expect("remembering should succeed");

    let bench = buried_while(bench, &project, naming).await;

    assert!(
        bench
            .store
            .project_names(std::slice::from_ref(&project))
            .await
            .expect("asking should succeed")
            .is_empty(),
        "a tombstone that did not wait for the name under way would leave it, for a project that is gone"
    );
    bench.cleanup().await;
}

#[tokio::test]
async fn a_project_s_tombstone_waits_for_its_trash_under_way() {
    let bench = OnPostgres::setup().await;
    let project = ProjectLink::from("project_loom");
    let mut trashing = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store
        .remember_trash(&mut trashing, &project, Version::of(2), true)
        .await
        .expect("remembering should succeed");

    let bench = buried_while(bench, &project, trashing).await;

    assert!(
        !bench
            .store
            .is_in_the_trash(&project)
            .await
            .expect("asking should succeed"),
        "a tombstone that did not wait for the trash under way would leave it, for a project that is gone"
    );
    bench.cleanup().await;
}
