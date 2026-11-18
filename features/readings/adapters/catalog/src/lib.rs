#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod kept;
mod memory;
mod sample;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemorySceneTextCatalog;
pub use sample::SampleBook;

#[cfg(feature = "postgres")]
pub use postgres::PostgresSceneTextCatalog;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteSceneTextCatalog;
