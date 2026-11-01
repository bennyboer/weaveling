use std::sync::Arc;

use async_trait::async_trait;
use email::EmailAddress;

use crate::InvitationChanges;
use eventsourcing::AccountId;
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, InvitationSummary, InvitationTokenDigest,
    Person, ProjectLink, ProjectName,
};
use live::{Broadcaster, Teller};
use time::OffsetDateTime;
use transacting::Transaction;

pub struct TellingInvitations {
    kept: Arc<dyn InvitationCatalog>,
    teller: Arc<Teller<InvitationChanges>>,
}

impl TellingInvitations {
    pub fn around(
        kept: Arc<dyn InvitationCatalog>,
        broadcaster: Broadcaster<InvitationChanges>,
    ) -> Self {
        Self {
            kept,
            teller: Arc::new(Teller::new(broadcaster)),
        }
    }
}

#[async_trait]
impl InvitationCatalog for TellingInvitations {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &InvitationSummary,
    ) -> Result<(), CatalogError> {
        self.kept.remember(transaction, summary).await?;
        let (teller, to, of) = (
            self.teller.clone(),
            summary.to.clone(),
            summary.id.to_string(),
        );
        transaction.after_commit(async move {
            teller.tell(InvitationChanges(to), &of, ());
        });

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

    async fn with_digest(
        &self,
        digest: &InvitationTokenDigest,
    ) -> Result<Option<InvitationSummary>, CatalogError> {
        self.kept.with_digest(digest).await
    }

    async fn due_for_retiring(
        &self,
        now: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<InvitationId>, CatalogError> {
        self.kept.due_for_retiring(now, at_most).await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept.forget(transaction, invitation, at).await
    }

    async fn forget_invitation_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.kept
            .forget_invitation_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
    ) -> Result<(), CatalogError> {
        self.kept.remember_person(transaction, person).await
    }

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
        self.kept.people(accounts).await
    }

    async fn remember_project_name(
        &self,
        transaction: &mut Transaction,
        named: &ProjectName,
    ) -> Result<(), CatalogError> {
        self.kept.remember_project_name(transaction, named).await
    }

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept
            .lay_project_tombstone(transaction, project, at)
            .await
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        self.kept.has_project_tombstone(project).await
    }

    async fn forget_project_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.kept
            .forget_project_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    async fn project_names(
        &self,
        projects: &[ProjectLink],
    ) -> Result<Vec<ProjectName>, CatalogError> {
        self.kept.project_names(projects).await
    }
}
