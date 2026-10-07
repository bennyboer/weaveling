mod enqueuing;
mod memory;

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(test)]
mod suite;

pub use enqueuing::PassageMessageMapping;
pub use memory::InMemoryPassageStore;

#[cfg(feature = "postgres")]
pub use postgres::{COMPACT_AFTER, PostgresPassageStore, migrations};
