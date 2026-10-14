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

pub use memory::InMemoryWordCountCatalog;

#[cfg(feature = "postgres")]
pub use postgres::PostgresWordCountCatalog;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteWordCountCatalog;
