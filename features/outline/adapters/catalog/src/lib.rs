mod memory;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;
pub mod suite;

pub use memory::InMemoryOutlineCatalog;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresOutlineCatalog, migrations};
