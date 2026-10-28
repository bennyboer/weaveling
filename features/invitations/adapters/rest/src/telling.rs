use std::sync::Arc;

use async_trait::async_trait;
use email::EmailAddress;
use eventsourcing::AccountId;
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, InvitationSummary, Person, ProjectLink,
    ProjectName,
};
use live::{Broadcaster, Teller};
use time::OffsetDateTime;

pub struct TellingInvitations {
    kept: Arc<dyn InvitationCatalog>,
    teller: Teller,
}

impl TellingInvitations {
    pub fn around(kept: Arc<dyn InvitationCatalog>, broadcaster: Broadcaster) -> Self {
        Self {
            kept,
            teller: Teller::new(broadcaster),
        }
    }
}

#[async_trait]
impl InvitationCatalog for TellingInvitations {
    async fn remember(&self, summary: &InvitationSummary) -> Result<(), CatalogError> {
        self.kept.remember(summary).await?;
        self.teller
            .tell(summary.to.as_str(), &summary.id.to_string(), ());

        Ok(())
    }

    async fn listed_after(
        &self,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<InvitationSummary>, CatalogError> {
        self.kept.listed_after(after, at_most).await
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<InvitationSummary>, CatalogError> {
        self.kept.in_project_after(project, after, at_most).await
    }

    async fn open_to(&self, to: &EmailAddress) -> Result<Vec<InvitationSummary>, CatalogError> {
        self.kept.open_to(to).await
    }

    async fn due_for_retiring(
        &self,
        now: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<InvitationId>, CatalogError> {
        self.kept.due_for_retiring(now, at_most).await
    }

    async fn forget(&self, invitation: &InvitationId) -> Result<(), CatalogError> {
        self.kept.forget(invitation).await
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

    async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept.lay_project_tombstone(project, at).await
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        self.kept.has_project_tombstone(project).await
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

    async fn project_names(
        &self,
        projects: &[ProjectLink],
    ) -> Result<Vec<ProjectName>, CatalogError> {
        self.kept.project_names(projects).await
    }
}
