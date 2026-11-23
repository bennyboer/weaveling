use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use eventsourcing::Agent;
use feedback_core::{
    Asked, CatalogError, CataloguedFeedback, Counted, FeedbackCatalog, FeedbackId, Order, Page,
    Pending, ProjectLink, Status, Subject,
};
use transacting::{Failure, Transaction};

#[derive(Debug, Default)]
pub struct InMemoryFeedbackCatalog {
    pieces: Arc<RwLock<Vec<CataloguedFeedback>>>,
}

impl InMemoryFeedbackCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Vec<CataloguedFeedback>> {
        self.pieces.read().expect("feedback catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Vec<CataloguedFeedback>> {
        self.pieces.write().expect("feedback catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let pieces = self.read().clone();
        let into_pieces = self.pieces.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_pieces.write().expect("feedback catalog lock poisoned") = pieces;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl FeedbackCatalog for InMemoryFeedbackCatalog {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        piece: &CataloguedFeedback,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut pieces = self.write();

        match pieces.iter_mut().find(|kept| kept.id == piece.id) {
            Some(kept) => *kept = piece.clone(),
            None => pieces.push(piece.clone()),
        }

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        piece: &FeedbackId,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write().retain(|kept| &kept.id != piece);

        Ok(())
    }

    async fn piece(&self, piece: &FeedbackId) -> Result<Option<CataloguedFeedback>, CatalogError> {
        Ok(self.read().iter().find(|kept| &kept.id == piece).cloned())
    }

    async fn drafts_of(
        &self,
        asked: &Asked,
        writer: &Agent,
    ) -> Result<Vec<CataloguedFeedback>, CatalogError> {
        let mut drafts: Vec<CataloguedFeedback> = self
            .read()
            .iter()
            .filter(|piece| piece.is_draft() && &piece.written_by == writer && asked.holds(piece))
            .cloned()
            .collect();
        drafts.sort_by_key(|piece| (piece.written_at, piece.id.to_string()));

        Ok(drafts)
    }

    async fn submitted(
        &self,
        asked: &Asked,
        status: Option<Status>,
        page: &Page,
    ) -> Result<Vec<CataloguedFeedback>, CatalogError> {
        let mut found: Vec<CataloguedFeedback> = self
            .read()
            .iter()
            .filter(|piece| {
                !piece.is_draft()
                    && asked.holds(piece)
                    && status.is_none_or(|status| status.holds(piece))
            })
            .cloned()
            .collect();
        found.sort_by_key(|piece| (piece.submitted_at, piece.written_at, piece.id.to_string()));
        if page.order == Order::Newest {
            found.reverse();
        }
        let from = page
            .after
            .as_ref()
            .and_then(|after| {
                found
                    .iter()
                    .position(|piece| &piece.id.to_string() == after)
            })
            .map_or(0, |at| at + 1);

        Ok(found
            .into_iter()
            .skip(from)
            .take(page.at_most.unwrap_or(usize::MAX))
            .collect())
    }

    async fn pending(&self, asked: &Asked) -> Result<Pending, CatalogError> {
        let mut pending = Pending::default();
        for piece in self
            .read()
            .iter()
            .filter(|piece| !piece.is_draft() && !piece.done && asked.holds(piece))
        {
            if let Some(scene) = piece.about.in_scene() {
                *pending.scenes.entry(scene.clone()).or_insert(0) += 1;
            }
            if let Some(section) = piece.about.on_section() {
                *pending.sections.entry(section.clone()).or_insert(0) += 1;
            }
            pending.total += 1;
        }

        Ok(pending)
    }

    async fn drafts_on(
        &self,
        subject: &Subject,
        at_most: usize,
    ) -> Result<Vec<FeedbackId>, CatalogError> {
        Ok(self
            .read()
            .iter()
            .filter(|piece| piece.is_draft() && &piece.about.subject() == subject)
            .take(at_most)
            .map(|piece| piece.id)
            .collect())
    }

    async fn pieces_on(
        &self,
        subject: &Subject,
        at_most: usize,
    ) -> Result<Vec<FeedbackId>, CatalogError> {
        Ok(self
            .read()
            .iter()
            .filter(|piece| &piece.about.subject() == subject)
            .take(at_most)
            .map(|piece| piece.id)
            .collect())
    }

    async fn givers_in(&self, project: &ProjectLink) -> Result<Vec<Agent>, CatalogError> {
        let pieces = self.read();
        let mut given: Vec<&CataloguedFeedback> = pieces
            .iter()
            .filter(|piece| !piece.is_draft() && &piece.project == project)
            .collect();
        given.sort_by_key(|piece| (piece.submitted_at, piece.written_at, piece.id.to_string()));
        let mut givers: Vec<Agent> = Vec::new();
        for piece in given {
            if !givers.contains(&piece.written_by) {
                givers.push(piece.written_by.clone());
            }
        }

        Ok(givers)
    }

    async fn counted_on(
        &self,
        subjects: &[Subject],
        asker: &Agent,
    ) -> Result<Vec<Counted>, CatalogError> {
        let pieces = self.read();

        Ok(subjects
            .iter()
            .map(|subject| {
                let on: Vec<&CataloguedFeedback> = pieces
                    .iter()
                    .filter(|piece| &piece.about.subject() == subject)
                    .collect();

                Counted {
                    not_done: on
                        .iter()
                        .filter(|piece| !piece.is_draft() && !piece.done)
                        .count(),
                    your_drafts: on
                        .iter()
                        .filter(|piece| piece.is_draft() && &piece.written_by == asker)
                        .count(),
                    subject: subject.clone(),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryFeedbackCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemoryFeedbackCatalog;

        async fn setup() -> Self {
            Self(InMemoryFeedbackCatalog::new(), InMemoryTransactions::new())
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::feedback_suite::feedback_conformance_tests!(InMemory);
}
