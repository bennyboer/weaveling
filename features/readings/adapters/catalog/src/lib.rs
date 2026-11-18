#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod kept;
mod memory;
mod outline_memory;
#[cfg(test)]
mod outline_suite;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
mod outline_postgres;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
mod outline_sqlite;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemorySceneTextCatalog;
pub use outline_memory::InMemoryOutlineCatalog;

#[cfg(feature = "postgres")]
pub use outline_postgres::PostgresOutlineCatalog;
#[cfg(feature = "postgres")]
pub use postgres::PostgresSceneTextCatalog;

#[cfg(feature = "sqlite")]
pub use outline_sqlite::SqliteOutlineCatalog;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteSceneTextCatalog;
