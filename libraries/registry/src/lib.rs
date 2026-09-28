mod memory;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;
pub mod suite;

pub use memory::InMemoryRegistry;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresRegistry, migrations};

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
}
