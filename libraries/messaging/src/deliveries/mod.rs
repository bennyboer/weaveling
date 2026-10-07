mod memory;
#[cfg(test)]
pub mod suite;

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

pub use memory::InMemoryDeliveries;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresDeliveries, migrations};
