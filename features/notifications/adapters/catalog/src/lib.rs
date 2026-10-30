mod memory;
pub mod suite;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod kept;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemoryNoticeCatalog;

#[cfg(feature = "postgres")]
pub use postgres::PostgresNoticeCatalog;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteNoticeCatalog;
