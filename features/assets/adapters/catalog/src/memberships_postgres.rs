use assets_core::{CatalogError, Membership, MembershipCatalog, ProjectLink};
use async_trait::async_trait;
use eventsourcing::AccountId;
use transacting::Transaction;
use transacting::postgres::connection;

use crate::written::unreachable;

const REMEMBER: &str =
    "INSERT INTO asset_memberships (project, account) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET: &str = "DELETE FROM asset_memberships WHERE project = $1 AND account = $2";

const FORGET_PROJECT: &str = "DELETE FROM asset_memberships WHERE project = $1";

const SHARED: &str = "
    SELECT EXISTS (
        SELECT 1 FROM asset_memberships one
        JOIN asset_memberships other ON other.project = one.project
        WHERE one.account = $1 AND other.account = $2
    )
";

/// Who is a member of which project, on Postgres, kept in the catalog's own schema beside the assets.
#[derive(Debug, Default)]
pub struct PostgresMembershipCatalog;

impl PostgresMembershipCatalog {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MembershipCatalog for PostgresMembershipCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(membership.project.to_string())
            .bind(membership.account.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
    ) -> Result<(), CatalogError> {
        sqlx::query(FORGET)
            .bind(membership.project.to_string())
            .bind(membership.account.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_project(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        sqlx::query(FORGET_PROJECT)
            .bind(project.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn share_a_project(
        &self,
        transaction: &mut Transaction,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, CatalogError> {
        sqlx::query_scalar(SHARED)
            .bind(one.to_string())
            .bind(other.to_string())
            .fetch_one(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use sqlx::PgPool;
    use test_harness::PostgresFixture;
    use transacting::{PostgresTransactions, Transactions};

    use super::*;
    use crate::memberships_suite::Workbench;
    use crate::postgres;

    struct OnPostgres {
        fixture: PostgresFixture,
        store: PostgresMembershipCatalog,
        transactions: PostgresTransactions,
    }

    #[async_trait]
    impl Workbench for OnPostgres {
        type Store = PostgresMembershipCatalog;

        async fn setup() -> Self {
            let fixture = PostgresFixture::setup().await;
            let pool: PgPool = fixture.create_schema("assets").await;
            postgres::migrations()
                .run(&pool)
                .await
                .expect("the schema should lay down in an empty namespace");

            Self {
                fixture,
                store: PostgresMembershipCatalog::new(),
                transactions: PostgresTransactions::new(pool),
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

    crate::membership_conformance_tests!(OnPostgres);
}
