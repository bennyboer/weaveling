mod memory;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

pub use memory::InMemoryAppearanceCatalog;

#[cfg(feature = "postgres")]
pub use postgres::{PostgresAppearanceCatalog, migrations};
