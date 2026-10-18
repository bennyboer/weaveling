use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use scenes_core::{ProjectLink, QuotationRestylingTaskError, QuotationRestylingTasks};

#[derive(Debug, Default)]
pub struct InMemoryQuotationRestylingTasks {
    latest: Mutex<HashMap<ProjectLink, u64>>,
}

impl InMemoryQuotationRestylingTasks {
    pub fn new() -> Self {
        Self::default()
    }

    fn latest(&self) -> MutexGuard<'_, HashMap<ProjectLink, u64>> {
        self.latest
            .lock()
            .expect("quotation restyling tasks lock poisoned")
    }
}

#[async_trait]
impl QuotationRestylingTasks for InMemoryQuotationRestylingTasks {
    async fn claim(
        &self,
        project: &ProjectLink,
        change: u64,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let mut latest = self.latest();
        let known = latest.entry(project.clone()).or_insert(change);
        if *known > change {
            return Ok(false);
        }
        *known = change;

        Ok(true)
    }
}
