use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use thiserror::Error;
use time::OffsetDateTime;

use crate::id::NoticeId;
use crate::notice::{About, Notice, ProjectLink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeSummary {
    pub id: NoticeId,
    pub version: Version,
    pub account: AccountId,
    pub about: About,
    pub noted_at: OffsetDateTime,
    pub dismissed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub account: AccountId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectName {
    pub project: ProjectLink,
    pub version: Version,
    pub name: String,
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

    async fn remember_person(&self, person: &Person) -> Result<(), CatalogError>;

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError>;

    async fn remember_project_name(&self, named: &ProjectName) -> Result<(), CatalogError>;

    async fn project_names(
        &self,
        projects: &[ProjectLink],
    ) -> Result<Vec<ProjectName>, CatalogError>;

    async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError>;

    async fn accounts_noticed_about(
        &self,
        project: &ProjectLink,
    ) -> Result<Vec<AccountId>, CatalogError>;

    async fn due_for_forgetting(
        &self,
        dismissed_before: OffsetDateTime,
        noted_before: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<NoticeId>, CatalogError>;

    async fn forget(&self, notice: &NoticeId) -> Result<(), CatalogError>;

    async fn forget_project_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;
}

impl NoticeSummary {
    pub fn of(id: NoticeId, version: Version, notice: &Notice) -> Self {
        Self {
            id,
            version,
            account: notice.account().clone(),
            about: notice.about().clone(),
            noted_at: notice.noted_at(),
            dismissed_at: notice.dismissed_at(),
        }
    }
}
