use appearances_core::{AppearanceCatalog, CatalogError, Place, ProjectLink, Subject};
use async_trait::async_trait;
use time::OffsetDateTime;
use transacting::{Failure, InMemoryTransactions, Transactions};

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
pub trait Committed {
    /// Stores a subject as appearing in a place of the subject's own project, if that project is known.
    async fn remember_committed(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError>;

    async fn forget_committed(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError>;

    async fn forget_subject_committed(
        &self,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_place_committed(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError>;

    async fn forget_tombstones_laid_before_committed(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn remember_project_committed(
        &self,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;
}

#[async_trait]
impl<C: AppearanceCatalog + ?Sized> Committed for C {
    async fn remember_committed(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        let mut transaction = InMemoryTransactions::new()
            .begin()
            .await
            .map_err(unreachable)?;
        if let Some(project) = self.project_of(subject).await? {
            self.remember_place_project(&mut transaction, place, &project)
                .await?;
        }
        self.remember(&mut transaction, subject, place, version)
            .await?;
        transaction.finish().await.map_err(unreachable)
    }

    async fn forget_committed(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        let mut transaction = InMemoryTransactions::new()
            .begin()
            .await
            .map_err(unreachable)?;
        self.forget(&mut transaction, subject, place, version)
            .await?;
        transaction.finish().await.map_err(unreachable)
    }

    async fn forget_subject_committed(
        &self,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let mut transaction = InMemoryTransactions::new()
            .begin()
            .await
            .map_err(unreachable)?;
        self.forget_subject(&mut transaction, subject, at).await?;
        transaction.finish().await.map_err(unreachable)
    }

    async fn forget_place_committed(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError> {
        let mut transaction = InMemoryTransactions::new()
            .begin()
            .await
            .map_err(unreachable)?;
        let appeared = self.forget_place(&mut transaction, place, at).await?;
        transaction.finish().await.map_err(unreachable)?;

        Ok(appeared)
    }

    async fn forget_tombstones_laid_before_committed(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let mut transaction = InMemoryTransactions::new()
            .begin()
            .await
            .map_err(unreachable)?;
        let forgotten = self
            .forget_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        transaction.finish().await.map_err(unreachable)?;

        Ok(forgotten)
    }

    async fn remember_project_committed(
        &self,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        let mut transaction = InMemoryTransactions::new()
            .begin()
            .await
            .map_err(unreachable)?;
        self.remember_project(&mut transaction, subject, project)
            .await?;
        transaction.finish().await.map_err(unreachable)
    }
}
