mod memory;
mod rule;
pub mod suite;

use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

pub use memory::InMemoryThrottles;
pub use rule::{Rule, Tries};

#[derive(Debug, Error)]
pub enum ThrottlesError {
    #[error("the throttles could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait Throttles: Send + Sync {
    async fn tries(&self, guarded: &str, about: &str) -> Result<Option<Tries>, ThrottlesError>;

    async fn count(
        &self,
        guarded: &str,
        about: &str,
        at: OffsetDateTime,
    ) -> Result<Tries, ThrottlesError>;

    async fn clear(&self, guarded: &str, about: &str) -> Result<(), ThrottlesError>;
}

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;
#[cfg(feature = "postgres")]
pub use postgres::PostgresThrottles;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteThrottles;
