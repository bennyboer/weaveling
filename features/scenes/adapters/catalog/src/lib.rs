mod memory;
mod quotation_restyling_tasks;
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
pub use quotation_restyling_tasks::InMemoryQuotationRestylingTasks;

#[cfg(feature = "postgres")]
pub use postgres::{PostgresQuotationRestylingTasks, PostgresSceneCatalog};

#[cfg(feature = "sqlite")]
pub use sqlite::{SqliteQuotationRestylingTasks, SqliteSceneCatalog};
