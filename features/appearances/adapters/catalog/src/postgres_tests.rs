use std::sync::Arc;

use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, Place, SceneLink, Subject};
use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::{Duration, OffsetDateTime};
use transacting::{PostgresTransactions, Transactions, Tried};

use crate::postgres;
use crate::postgres::PostgresAppearanceCatalog;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    catalog: PostgresAppearanceCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Catalog = PostgresAppearanceCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("appearances").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            catalog: PostgresAppearanceCatalog::new(pool.clone()),
            pool,
        }
    }

    fn catalog(&self) -> &Self::Catalog {
        &self.catalog
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::suite::conformance_tests!(OnPostgres);

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

#[derive(Clone, Copy)]
enum Gone {
    Subject,
    Place,
}

async fn remembered_while_gone(gone: Gone) -> Vec<Place> {
    let bench = OnPostgres::setup().await;
    let catalog = Arc::new(PostgresAppearanceCatalog::new(bench.pool.clone()));
    let transactions = Arc::new(PostgresTransactions::new(bench.pool.clone()));
    let mut left = Vec::new();

    for nth in 0..40 {
        let subject = Subject::Idea(IdeaLink::from(format!("idea_{nth}")));
        let place = Place::Scene(SceneLink::from(format!("scene_{nth}")));
        let (remembering, forgetting) = (catalog.clone(), catalog.clone());
        let (remembered_in, forgotten_in) = (transactions.clone(), transactions.clone());
        let (of, at_place) = (subject.clone(), place.clone());
        let (gone_subject, gone_place) = (subject.clone(), place);
        let (remembered, forgotten) = tokio::join!(
            tokio::spawn(async move {
                transacting::retrying!(&*remembered_in, |transaction| {
                    remembering
                        .remember(transaction, &of, &at_place, 1)
                        .await
                        .map_err(Tried::Refused)?;
                    Ok::<_, Tried<CatalogError>>(())
                })
            }),
            tokio::spawn(async move {
                transacting::retrying!(&*forgotten_in, |transaction| {
                    match gone {
                        Gone::Subject => forgetting
                            .forget_subject(transaction, &gone_subject, at(5_000))
                            .await
                            .map_err(Tried::Refused)?,
                        Gone::Place => {
                            forgetting
                                .forget_place(transaction, &gone_place, at(5_000))
                                .await
                                .map_err(Tried::Refused)?;
                        }
                    }
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
        left.extend(
            catalog
                .places_of(&subject)
                .await
                .expect("asking should succeed"),
        );
    }

    bench.cleanup().await;
    left
}

#[tokio::test(flavor = "multi_thread")]
async fn an_appearance_racing_its_idea_s_discard_never_brings_the_idea_back() {
    assert!(
        remembered_while_gone(Gone::Subject).await.is_empty(),
        "an appearance written while its idea was forgotten must not outlive the tombstone"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_appearance_racing_its_place_going_never_brings_the_place_back() {
    assert!(
        remembered_while_gone(Gone::Place).await.is_empty(),
        "an appearance written while its place went must not outlive the tombstone"
    );
}
