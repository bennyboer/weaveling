mod account_limits_memory;
#[cfg(feature = "postgres")]
mod account_limits_postgres;
#[cfg(feature = "sqlite")]
mod account_limits_sqlite;
#[cfg(test)]
mod account_limits_suite;
mod memberships_memory;
#[cfg(feature = "postgres")]
mod memberships_postgres;
#[cfg(feature = "sqlite")]
mod memberships_sqlite;
#[cfg(test)]
mod memberships_suite;
mod memory;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use account_limits_memory::InMemoryAccountLimits;
#[cfg(feature = "postgres")]
pub use account_limits_postgres::PostgresAccountLimits;
#[cfg(feature = "sqlite")]
pub use account_limits_sqlite::SqliteAccountLimits;
pub use memberships_memory::InMemoryMembershipCatalog;
#[cfg(feature = "postgres")]
pub use memberships_postgres::PostgresMembershipCatalog;
#[cfg(feature = "sqlite")]
pub use memberships_sqlite::SqliteMembershipCatalog;
pub use memory::InMemoryAssetCatalog;

#[cfg(feature = "postgres")]
pub use postgres::PostgresAssetCatalog;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteAssetCatalog;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod written;
