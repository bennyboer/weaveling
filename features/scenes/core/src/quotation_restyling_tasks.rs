use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::ProjectLink;

#[derive(Debug, Error)]
pub enum QuotationRestylingTaskError {
    #[error("the quotation restyling tasks could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait QuotationRestylingTasks: Send + Sync {
    async fn claim(
        &self,
        project: &ProjectLink,
        change: u64,
    ) -> Result<bool, QuotationRestylingTaskError>;
}
