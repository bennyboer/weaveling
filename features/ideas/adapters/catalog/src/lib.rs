mod memory;
pub mod suite;

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

pub use memory::InMemoryIdeaCatalog;

#[cfg(feature = "postgres")]
pub use postgres::{PostgresIdeaCatalog, migrations};
