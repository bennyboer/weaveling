use async_trait::async_trait;
use eventsourcing::Version;
use notifications_core::{NoticeCatalog, ProjectLink, ProjectName};
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{PostgresTransactions, Transactions};

use crate::postgres;
use crate::postgres::PostgresNoticeCatalog;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    transactions: PostgresTransactions,
    store: PostgresNoticeCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresNoticeCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("notifications").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            store: PostgresNoticeCatalog::new(pool),
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

    tokio::join!(
        async {
            let mut burying = bench
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            bench
                .store
                .lay_project_tombstone(&mut burying, &project, OffsetDateTime::UNIX_EPOCH)
                .await
                .expect("laying a tombstone should succeed");
            burying.finish().await.expect("committing should succeed");
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            naming.finish().await.expect("committing should succeed");
        }
    );

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
