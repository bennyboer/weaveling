mod feedback_memory;
#[cfg(feature = "postgres")]
mod feedback_postgres;
#[cfg(feature = "sqlite")]
mod feedback_sqlite;
#[cfg(test)]
mod feedback_suite;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod kept;
mod names_memory;
#[cfg(feature = "postgres")]
mod names_postgres;
#[cfg(feature = "sqlite")]
mod names_sqlite;
#[cfg(test)]
mod names_suite;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;
mod reading_copies_memory;
#[cfg(feature = "postgres")]
mod reading_copies_postgres;
#[cfg(feature = "sqlite")]
mod reading_copies_sqlite;
#[cfg(test)]
mod reading_copies_suite;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;
#[cfg(test)]
mod suite;

pub use feedback_memory::InMemoryFeedbackCatalog;
pub use names_memory::InMemoryAccountNameCatalog;
pub use reading_copies_memory::InMemoryReadingCopyCatalog;

#[cfg(feature = "postgres")]
pub use feedback_postgres::PostgresFeedbackCatalog;
#[cfg(feature = "postgres")]
pub use names_postgres::PostgresAccountNameCatalog;
#[cfg(feature = "postgres")]
pub use reading_copies_postgres::PostgresReadingCopyCatalog;

#[cfg(feature = "sqlite")]
pub use feedback_sqlite::SqliteFeedbackCatalog;
#[cfg(feature = "sqlite")]
pub use names_sqlite::SqliteAccountNameCatalog;
#[cfg(feature = "sqlite")]
pub use reading_copies_sqlite::SqliteReadingCopyCatalog;
