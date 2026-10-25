use std::error::Error;

use async_trait::async_trait;
use quotation::QuotationMarks;
use thiserror::Error;

use crate::ProjectLink;

#[derive(Debug, Error)]
pub enum QuotationRestylingTaskError {
    #[error("the quotation restyling tasks could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotationRestyling {
    pub project: String,
    pub style: QuotationMarks,
    pub change: u64,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotationRestylingProgress {
    pub change: u64,
    pub total: u64,
    pub done: u64,
    pub finished: bool,
}

#[async_trait]
pub trait QuotationRestylingTasks: Send + Sync {
    async fn begin(
        &self,
        project: &ProjectLink,
        change: u64,
        total: u64,
    ) -> Result<bool, QuotationRestylingTaskError>;

    async fn claim(
        &self,
        project: &ProjectLink,
        change: u64,
    ) -> Result<bool, QuotationRestylingTaskError>;

    async fn progressed(
        &self,
        project: &ProjectLink,
        change: u64,
        done: u64,
        finished: bool,
    ) -> Result<(), QuotationRestylingTaskError>;

    async fn progress(
        &self,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingProgress>, QuotationRestylingTaskError>;
}
