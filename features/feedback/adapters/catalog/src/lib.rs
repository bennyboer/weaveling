mod feedback_memory;
#[cfg(feature = "postgres")]
mod feedback_postgres;
#[cfg(test)]
mod feedback_suite;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod kept;
mod names_memory;
#[cfg(feature = "postgres")]
mod names_postgres;
#[cfg(test)]
mod names_suite;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;
mod reading_copies_memory;
#[cfg(feature = "postgres")]
mod reading_copies_postgres;
#[cfg(test)]
mod reading_copies_suite;
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
