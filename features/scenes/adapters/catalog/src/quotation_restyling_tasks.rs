use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use scenes_core::{
    Page, ProjectLink, QuotationRestylingProgress, QuotationRestylingStanding,
    QuotationRestylingTaskError, QuotationRestylingTasks,
};
use transacting::Transaction;

#[derive(Debug, Clone)]
struct Task {
    progress: QuotationRestylingProgress,
    cursor: Option<String>,
}

#[derive(Debug, Default)]
pub struct InMemoryQuotationRestylingTasks {
    latest: Arc<Mutex<HashMap<ProjectLink, Task>>>,
}

impl InMemoryQuotationRestylingTasks {
    pub fn new() -> Self {
        Self::default()
    }

    fn latest(&self) -> MutexGuard<'_, HashMap<ProjectLink, Task>> {
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

#[async_trait]
impl QuotationRestylingTasks for InMemoryQuotationRestylingTasks {
    async fn begin(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        total: u64,
        cursor: Option<&str>,
    ) -> Result<bool, QuotationRestylingTaskError> {
        self.undo_on_rollback(transaction)?;
        let mut latest = self.latest();
        if latest
            .get(project)
            .is_some_and(|known| known.progress.change >= change)
        {
            return Ok(false);
        }
        latest.insert(
            project.clone(),
            Task {
                progress: QuotationRestylingProgress {
                    change,
                    total,
                    done: 0,
                    finished: false,
                },
                cursor: cursor.map(str::to_owned),
            },
        );

        Ok(true)
    }

    async fn standing(
        &self,
        _transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingStanding>, QuotationRestylingTaskError> {
        Ok(self
            .latest()
            .get(project)
            .map(|known| QuotationRestylingStanding {
                change: known.progress.change,
                cursor: known.cursor.clone(),
                finished: known.progress.finished,
            }))
    }

    async fn progressed(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        page: Page<'_>,
        finished: bool,
    ) -> Result<bool, QuotationRestylingTaskError> {
        self.undo_on_rollback(transaction)?;
        let mut latest = self.latest();
        let Some(known) = latest.get_mut(project).filter(|known| {
            known.progress.change == change
                && !known.progress.finished
                && known.cursor.as_deref() == page.from
        }) else {
            return Ok(false);
        };
        known.progress.done += page.done;
        known.progress.finished = finished;
        known.cursor = page.to.map(str::to_owned);

        Ok(true)
    }

    async fn progress(
        &self,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingProgress>, QuotationRestylingTaskError> {
        Ok(self.latest().get(project).map(|known| known.progress))
    }
}
