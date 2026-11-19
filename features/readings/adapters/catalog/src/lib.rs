mod contents_memory;
#[cfg(test)]
mod contents_suite;
mod copies_memory;
#[cfg(test)]
mod copies_suite;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod kept;
mod memory;
mod names_memory;
#[cfg(test)]
mod names_suite;
mod outline_memory;
#[cfg(test)]
mod outline_suite;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
mod contents_postgres;
#[cfg(feature = "postgres")]
mod copies_postgres;
#[cfg(feature = "postgres")]
mod names_postgres;
#[cfg(feature = "postgres")]
mod outline_postgres;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
mod contents_sqlite;
#[cfg(feature = "sqlite")]
mod copies_sqlite;
#[cfg(feature = "sqlite")]
mod names_sqlite;
#[cfg(feature = "sqlite")]
mod outline_sqlite;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use contents_memory::InMemoryReadingCopyContents;
pub use copies_memory::InMemoryReadingCopyCatalog;
pub use memory::InMemorySceneTextCatalog;
pub use names_memory::InMemoryAccountNameCatalog;
pub use outline_memory::InMemoryOutlineCatalog;

#[cfg(feature = "postgres")]
pub use contents_postgres::PostgresReadingCopyContents;
#[cfg(feature = "postgres")]
pub use copies_postgres::PostgresReadingCopyCatalog;
#[cfg(feature = "postgres")]
pub use names_postgres::PostgresAccountNameCatalog;
#[cfg(feature = "postgres")]
pub use outline_postgres::PostgresOutlineCatalog;
#[cfg(feature = "postgres")]
pub use postgres::PostgresSceneTextCatalog;

#[cfg(feature = "sqlite")]
pub use contents_sqlite::SqliteReadingCopyContents;
#[cfg(feature = "sqlite")]
pub use copies_sqlite::SqliteReadingCopyCatalog;
#[cfg(feature = "sqlite")]
pub use names_sqlite::SqliteAccountNameCatalog;
#[cfg(feature = "sqlite")]
pub use outline_sqlite::SqliteOutlineCatalog;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteSceneTextCatalog;
