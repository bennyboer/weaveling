mod memory;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(test)]
mod suite;

pub use memory::InMemoryProjectStore;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresProjectStore, migrations};
