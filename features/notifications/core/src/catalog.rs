use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use thiserror::Error;
use time::OffsetDateTime;

use crate::id::NoticeId;
use crate::notice::{About, Notice};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeSummary {
    pub id: NoticeId,
    pub version: Version,
    pub account: AccountId,
    pub about: About,
    pub noted_at: OffsetDateTime,
    pub dismissed: bool,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the list of notices could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait NoticeCatalog: Send + Sync {
    async fn remember(&self, summary: &NoticeSummary) -> Result<(), CatalogError>;

    async fn open_for(&self, account: &AccountId) -> Result<Vec<NoticeSummary>, CatalogError>;
}

impl NoticeSummary {
    pub fn of(id: NoticeId, version: Version, notice: &Notice) -> Self {
        Self {
            id,
            version,
            account: notice.account().clone(),
            about: notice.about().clone(),
            noted_at: notice.noted_at(),
            dismissed: notice.is_dismissed(),
        }
    }
}
