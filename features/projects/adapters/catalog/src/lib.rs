mod memory;
pub mod suite;

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemoryProjectCatalog;

#[cfg(feature = "postgres")]
pub use postgres::{PostgresProjectCatalog, migrations};

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteProjectCatalog;
