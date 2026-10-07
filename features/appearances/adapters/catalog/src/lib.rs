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

pub use memory::InMemoryAppearanceCatalog;

#[cfg(feature = "postgres")]
pub use postgres::PostgresAppearanceCatalog;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteAppearanceCatalog;
