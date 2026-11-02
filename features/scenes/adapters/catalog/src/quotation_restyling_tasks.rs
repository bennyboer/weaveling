use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use scenes_core::{
    ProjectLink, QuotationRestylingProgress, QuotationRestylingTaskError, QuotationRestylingTasks,
};
use transacting::Transaction;

#[derive(Debug, Default)]
pub struct InMemoryQuotationRestylingTasks {
    latest: Arc<Mutex<HashMap<ProjectLink, QuotationRestylingProgress>>>,
}

impl InMemoryQuotationRestylingTasks {
    pub fn new() -> Self {
        Self::default()
    }

    fn latest(&self) -> MutexGuard<'_, HashMap<ProjectLink, QuotationRestylingProgress>> {
        self.latest
            .lock()
            .expect("quotation restyling tasks lock poisoned")
    }

    fn undo_on_rollback(
        &self,
        transaction: &mut Transaction,
    ) -> Result<(), QuotationRestylingTaskError> {
        let (before, latest) = (self.latest().clone(), self.latest.clone());

        transacting::memory::undo_on_rollback(transaction, move || {
            *latest
                .lock()
                .expect("quotation restyling tasks lock poisoned") = before;
        })
        .map_err(|failure| QuotationRestylingTaskError::Backend(Box::new(failure)))
    }
}

fn unbegun(change: u64) -> QuotationRestylingProgress {
    QuotationRestylingProgress {
        change,
        total: 0,
        done: 0,
        finished: false,
    }
}

#[async_trait]
impl QuotationRestylingTasks for InMemoryQuotationRestylingTasks {
    async fn begin(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        total: u64,
    ) -> Result<bool, QuotationRestylingTaskError> {
        self.undo_on_rollback(transaction)?;
        let mut latest = self.latest();
        let known = latest.entry(project.clone()).or_insert(unbegun(change));
        if known.change > change {
            return Ok(false);
        }
        *known = QuotationRestylingProgress {
            total,
            ..unbegun(change)
        };

        Ok(true)
    }

    async fn claim(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
    ) -> Result<bool, QuotationRestylingTaskError> {
        self.undo_on_rollback(transaction)?;
        let mut latest = self.latest();
        let known = latest.entry(project.clone()).or_insert(unbegun(change));
        if known.change > change {
            return Ok(false);
        }
        known.change = change;

        Ok(true)
    }

    async fn progressed(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        done: u64,
        finished: bool,
    ) -> Result<(), QuotationRestylingTaskError> {
        self.undo_on_rollback(transaction)?;
        if let Some(known) = self
            .latest()
            .get_mut(project)
            .filter(|known| known.change == change)
        {
            known.done += done;
            known.finished = finished;
        }

        Ok(())
    }

    async fn progress(
        &self,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingProgress>, QuotationRestylingTaskError> {
        Ok(self.latest().get(project).copied())
    }
}
