mod memory;
pub mod suite;

pub use memory::InMemoryInvitationCatalog;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod outcome;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;
#[cfg(feature = "postgres")]
pub use postgres::PostgresInvitationCatalog;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteInvitationCatalog;
