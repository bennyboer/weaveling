use std::sync::Arc;

use async_trait::async_trait;
use eventsourcing::AccountId;
use live::{Broadcaster, Teller};
use notifications_core::{
    CatalogError, NoticeCatalog, NoticeId, NoticeSummary, Person, ProjectLink, ProjectName,
};
use time::OffsetDateTime;

use crate::NoticeChanges;

pub struct TellingNotices {
    kept: Arc<dyn NoticeCatalog>,
    teller: Teller<NoticeChanges>,
}

impl TellingNotices {
    pub fn around(kept: Arc<dyn NoticeCatalog>, broadcaster: Broadcaster<NoticeChanges>) -> Self {
        Self {
            kept,
            teller: Teller::new(broadcaster),
        }
    }
}

#[async_trait]
impl NoticeCatalog for TellingNotices {
    async fn remember(&self, summary: &NoticeSummary) -> Result<(), CatalogError> {
        self.kept.remember(summary).await?;
        self.teller.tell(
            NoticeChanges(summary.account.clone()),
            &summary.id.to_string(),
            (),
        );

        Ok(())
    }

    async fn open_for(&self, account: &AccountId) -> Result<Vec<NoticeSummary>, CatalogError> {
        self.kept.open_for(account).await
    }

    async fn remember_person(&self, person: &Person) -> Result<(), CatalogError> {
        self.kept.remember_person(person).await
    }

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
        self.kept.people(accounts).await
    }

    async fn remember_project_name(&self, named: &ProjectName) -> Result<(), CatalogError> {
        self.kept.remember_project_name(named).await
    }

    async fn project_names(
        &self,
        projects: &[ProjectLink],
    ) -> Result<Vec<ProjectName>, CatalogError> {
        self.kept.project_names(projects).await
    }

    async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept.lay_project_tombstone(project, at).await?;
        for account in self.kept.accounts_noticed_about(project).await? {
            self.teller
                .tell(NoticeChanges(account), project.as_str(), ());
        }

        Ok(())
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        self.kept.has_project_tombstone(project).await
    }

    async fn accounts_noticed_about(
        &self,
        project: &ProjectLink,
    ) -> Result<Vec<AccountId>, CatalogError> {
        self.kept.accounts_noticed_about(project).await
    }

    async fn due_for_forgetting(
        &self,
        dismissed_before: OffsetDateTime,
        noted_before: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<NoticeId>, CatalogError> {
        self.kept
            .due_for_forgetting(dismissed_before, noted_before, at_most)
            .await
    }

    async fn forget(&self, notice: &NoticeId) -> Result<(), CatalogError> {
        self.kept.forget(notice).await
    }

    async fn forget_project_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.kept
            .forget_project_tombstones_laid_before(before, at_most)
            .await
    }
}
