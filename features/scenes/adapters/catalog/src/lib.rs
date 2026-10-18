mod memory;
mod restyling_tasks;
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

pub use memory::InMemorySceneCatalog;
pub use restyling_tasks::InMemoryRestylingTasks;

#[cfg(feature = "postgres")]
pub use postgres::PostgresSceneCatalog;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteSceneCatalog;
