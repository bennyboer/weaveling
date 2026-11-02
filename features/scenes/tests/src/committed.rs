use async_trait::async_trait;
use scenes_core::{
    CatalogError, ProjectLink, QuotationRestylingTaskError, QuotationRestylingTasks, SceneCatalog,
    SceneId, SceneTitle,
};
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction, Transactions};

async fn begun() -> Transaction {
    InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed")
}

async fn finished(transaction: Transaction) {
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

#[async_trait]
pub trait Committed {
    async fn remember_committed(
        &self,
        id: SceneId,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;

    async fn retitle_committed(
        &self,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError>;

    async fn note_opening_committed(
        &self,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_committed(&self, id: SceneId, at: OffsetDateTime) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before_committed(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;
}

#[async_trait]
impl<C: SceneCatalog + ?Sized> Committed for C {
    async fn remember_committed(
        &self,
        id: SceneId,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        let mut transaction = begun().await;
        self.remember(&mut transaction, id, project).await?;
        finished(transaction).await;

        Ok(())
    }

    async fn retitle_committed(
        &self,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError> {
        let mut transaction = begun().await;
        self.retitle(&mut transaction, id, project, title, version)
            .await?;
        finished(transaction).await;

        Ok(())
    }

    async fn note_opening_committed(
        &self,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let mut transaction = begun().await;
        self.note_opening(&mut transaction, id, project, opening, at)
            .await?;
        finished(transaction).await;

        Ok(())
    }

    async fn forget_committed(&self, id: SceneId, at: OffsetDateTime) -> Result<(), CatalogError> {
        let mut transaction = begun().await;
        self.forget(&mut transaction, id, at).await?;
        finished(transaction).await;

        Ok(())
    }

    async fn forget_tombstones_laid_before_committed(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let mut transaction = begun().await;
        let forgotten = self
            .forget_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        finished(transaction).await;

        Ok(forgotten)
    }
}

#[async_trait]
pub trait TasksCommitted {
    async fn begin_committed(
        &self,
        project: &ProjectLink,
        change: u64,
        total: u64,
    ) -> Result<bool, QuotationRestylingTaskError>;

    async fn progressed_committed(
        &self,
        project: &ProjectLink,
        change: u64,
        done: u64,
        finished: bool,
    ) -> Result<(), QuotationRestylingTaskError>;
}

#[async_trait]
impl<K: QuotationRestylingTasks + ?Sized> TasksCommitted for K {
    async fn begin_committed(
        &self,
        project: &ProjectLink,
        change: u64,
        total: u64,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let mut transaction = begun().await;
        let begun = self.begin(&mut transaction, project, change, total).await?;
        finished(transaction).await;

        Ok(begun)
    }

    async fn progressed_committed(
        &self,
        project: &ProjectLink,
        change: u64,
        done: u64,
        finished_all: bool,
    ) -> Result<(), QuotationRestylingTaskError> {
        let mut transaction = begun().await;
        self.progressed(&mut transaction, project, change, done, finished_all)
            .await?;
        finished(transaction).await;

        Ok(())
    }
}
