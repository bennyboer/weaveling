mod memory;
#[cfg(test)]
pub mod suite;

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemoryDeliveries;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresDeliveries, migrations};

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteDeliveries;
