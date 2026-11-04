use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCountCatalog};
use test_harness::PostgresFixture;
use time::{Duration, OffsetDateTime};
use transacting::{PostgresTransactions, Transactions, Tried};

use crate::postgres;
use crate::postgres::PostgresWordCountCatalog;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    catalog: PostgresWordCountCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Catalog = PostgresWordCountCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("statistics").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            catalog: PostgresWordCountCatalog::new(pool.clone()),
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

#[tokio::test(flavor = "multi_thread")]
async fn keeping_and_forgetting_a_scene_at_once_never_leaves_a_deleted_scene_counted() {
    let bench = OnPostgres::setup().await;
    let catalog = Arc::new(PostgresWordCountCatalog::new(bench.pool.clone()));
    let transactions = Arc::new(PostgresTransactions::new(bench.pool.clone()));
    let project = ProjectLink::from("project_1");

    for nth in 0..40 {
        let scene = SceneLink::from(format!("scene_{nth}"));
        let (keeping, forgetting) = (catalog.clone(), catalog.clone());
        let (kept_in, forgotten_in) = (transactions.clone(), transactions.clone());
        let (kept_scene, forgotten_scene, of) = (scene.clone(), scene, project.clone());
        let (kept, forgotten) = tokio::join!(
            tokio::spawn(async move {
                transacting::retrying!(&*kept_in, |transaction| {
                    keeping
                        .keep(transaction, &kept_scene, &of, 42, at(1_000))
                        .await
                        .map_err(Tried::Refused)?;
                    Ok::<_, Tried<CatalogError>>(())
                })
            }),
            tokio::spawn(async move {
                transacting::retrying!(&*forgotten_in, |transaction| {
                    forgetting
                        .forget_scene(transaction, &forgotten_scene, at(5_000))
                        .await
                        .map_err(Tried::Refused)?;
                    Ok::<_, Tried<CatalogError>>(())
                })
            }),
        );
        kept.expect("keeping should not panic")
            .expect("keeping should succeed");
        forgotten
            .expect("forgetting should not panic")
            .expect("forgetting should succeed");
    }

    assert!(
        catalog
            .in_project(&project)
            .await
            .expect("asking should succeed")
            .is_empty(),
        "a count racing its scene's deletion must never bring the scene back"
    );
    bench.cleanup().await;
}
