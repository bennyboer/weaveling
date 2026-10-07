mod memory;
pub mod suite;

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

pub use memory::InMemoryProjectCatalog;

#[cfg(feature = "postgres")]
pub use postgres::{PostgresProjectCatalog, migrations};
