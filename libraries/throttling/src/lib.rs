mod forgetting;
mod memory;
mod rule;
pub mod suite;

use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

pub use forgetting::ForgetOldTries;
pub use memory::InMemoryThrottles;
pub use rule::{Admission, Rule, Tries, hour_of};

#[derive(Debug, Error)]
pub enum ThrottlesError {
    #[error("the throttles could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait Throttles: Send + Sync {
    async fn tries(&self, guarded: &str, about: &str) -> Result<Option<Tries>, ThrottlesError>;

    async fn admit(
        &self,
        guarded: &str,
        about: &str,
        rule: Rule,
        at: OffsetDateTime,
    ) -> Result<Admission, ThrottlesError>;

    async fn clear(&self, guarded: &str, about: &str) -> Result<(), ThrottlesError>;

    /// Forgets at most `at_most` counts whose last try was before `before`, the oldest first.
    /// A count a day old is reset on the next try anyway.
    /// So this only removes the record of whom the count was about.
    async fn forget_tried_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, ThrottlesError>;
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
