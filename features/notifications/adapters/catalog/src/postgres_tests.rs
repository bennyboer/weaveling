use async_trait::async_trait;
use eventsourcing::Version;
use notifications_core::{NoticeCatalog, NoticeSummary, ProjectLink, ProjectName};
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{PostgresTransactions, Transactions, Tried};

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
async fn a_project_s_tombstone_laid_while_its_name_is_under_way_leaves_no_name() {
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
            transacting::retrying!(&bench.transactions, |burying| {
                bench
                    .store
                    .lay_project_tombstone(burying, &project, OffsetDateTime::UNIX_EPOCH)
                    .await
                    .map_err(Tried::Refused)
            })
            .expect("laying a tombstone should succeed");
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            match naming.finish().await {
                Ok(()) | Err(transacting::Failure::LostRace) => {}
                Err(failed) => panic!("committing should succeed or lose the race: {failed:?}"),
            }
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

#[tokio::test]
async fn a_notice_forgotten_while_its_cataloguing_is_under_way_stays_forgotten() {
    let bench = OnPostgres::setup().await;
    let notice: NoticeSummary = crate::suite::a_summary("account_bo", 1_000, 1, false);
    let mut cataloguing = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store
        .remember(&mut cataloguing, &notice)
        .await
        .expect("remembering should succeed");

    tokio::join!(
        async {
            transacting::retrying!(&bench.transactions, |forgetting| {
                bench
                    .store
                    .forget(forgetting, &notice.id, OffsetDateTime::UNIX_EPOCH)
                    .await
                    .map_err(Tried::Refused)
            })
            .expect("forgetting should succeed");
        },
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            match cataloguing.finish().await {
                Ok(()) | Err(transacting::Failure::LostRace) => {}
                Err(failed) => panic!("committing should succeed or lose the race: {failed:?}"),
            }
        }
    );

    assert!(
        bench
            .store
            .open_for(&notice.account)
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a forget that did not wait would leave the row the cataloguing was writing"
    );
    bench.cleanup().await;
}
