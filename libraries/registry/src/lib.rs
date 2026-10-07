mod memory;
pub mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemoryRegistry;

#[cfg(feature = "postgres")]
pub use postgres::PostgresRegistry;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteRegistry;

use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("the registry could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait Registry: Send + Sync {
    async fn claim(&self, kind: &str, key: &str, id: &str) -> Result<String, RegistryError>;

    async fn holder(&self, kind: &str, key: &str) -> Result<Option<String>, RegistryError>;
}
