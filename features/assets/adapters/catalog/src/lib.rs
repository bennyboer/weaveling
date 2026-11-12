mod memberships_memory;
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

pub use memberships_memory::InMemoryMembershipCatalog;
pub use memory::InMemoryAssetCatalog;

#[cfg(feature = "postgres")]
pub use postgres::PostgresAssetCatalog;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteAssetCatalog;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod written;
