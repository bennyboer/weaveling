use std::error::Error;

use async_trait::async_trait;
use quotation::QuotationMarks;
use thiserror::Error;
use transacting::Transaction;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotationRestylingStanding {
    pub change: u64,
    pub cursor: Option<String>,
    pub finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotationRestylingProgress {
    pub change: u64,
    pub total: u64,
    pub done: u64,
    pub finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page<'a> {
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub done: u64,
}

#[async_trait]
pub trait QuotationRestylingTasks: Send + Sync {
    async fn begin(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        total: u64,
        cursor: Option<&str>,
    ) -> Result<bool, QuotationRestylingTaskError>;

    async fn standing(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingStanding>, QuotationRestylingTaskError>;

    async fn progressed(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        page: Page<'_>,
        finished: bool,
    ) -> Result<bool, QuotationRestylingTaskError>;

    async fn progress(
        &self,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingProgress>, QuotationRestylingTaskError>;
}
