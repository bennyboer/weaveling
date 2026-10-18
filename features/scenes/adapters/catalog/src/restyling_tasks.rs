use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use scenes_core::{ProjectLink, RestylingTaskError, RestylingTasks};

#[derive(Debug, Default)]
pub struct InMemoryRestylingTasks {
    latest: Mutex<HashMap<ProjectLink, u64>>,
}

impl InMemoryRestylingTasks {
    pub fn new() -> Self {
        Self::default()
    }

    fn latest(&self) -> MutexGuard<'_, HashMap<ProjectLink, u64>> {
        self.latest.lock().expect("restyling tasks lock poisoned")
    }
}

#[async_trait]
impl RestylingTasks for InMemoryRestylingTasks {
    async fn begin(&self, project: &ProjectLink, change: u64) -> Result<bool, RestylingTaskError> {
        let mut latest = self.latest();
        let known = latest.entry(project.clone()).or_insert(change);
        if *known > change {
            return Ok(false);
        }
        *known = change;

        Ok(true)
    }

    async fn is_latest(
        &self,
        project: &ProjectLink,
        change: u64,
    ) -> Result<bool, RestylingTaskError> {
        Ok(self
            .latest()
            .get(project)
            .is_none_or(|known| *known <= change))
    }
}
