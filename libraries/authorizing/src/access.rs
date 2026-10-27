use std::future;
use std::sync::Arc;

use tokio::sync::broadcast::{self, Receiver, Sender, error::RecvError};

const BACKLOG: usize = 256;

#[derive(Debug, Clone)]
pub struct AccessChanges {
    told: Sender<Arc<str>>,
}

pub struct AccessWatch {
    project: Arc<str>,
    told: Option<Receiver<Arc<str>>>,
}

impl AccessChanges {
    pub fn new() -> Self {
        Self {
            told: broadcast::channel(BACKLOG).0,
        }
    }

    pub fn tell(&self, project: &str) {
        _ = self.told.send(Arc::from(project));
    }

    pub fn watch(&self, project: &str) -> AccessWatch {
        AccessWatch {
            project: Arc::from(project),
            told: Some(self.told.subscribe()),
        }
    }
}

impl Default for AccessChanges {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessWatch {
    pub fn still() -> Self {
        Self {
            project: Arc::from(""),
            told: None,
        }
    }

    pub async fn changed(&mut self) {
        while let Some(told) = &mut self.told {
            match told.recv().await {
                Ok(project) if project == self.project => return,
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => return,
                Err(RecvError::Closed) => self.told = None,
            }
        }
        future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time;

    use super::*;

    async fn heard(watch: &mut AccessWatch) -> bool {
        time::timeout(Duration::from_millis(50), watch.changed())
            .await
            .is_ok()
    }

    #[tokio::test]
    async fn a_watch_hears_a_change_to_its_project_alone() {
        let changes = AccessChanges::new();
        let mut loom = changes.watch("project_loom");

        changes.tell("project_other");
        assert!(!heard(&mut loom).await);

        changes.tell("project_loom");
        assert!(heard(&mut loom).await);
    }

    #[tokio::test]
    async fn a_watch_that_fell_behind_takes_it_as_changed() {
        let changes = AccessChanges {
            told: broadcast::channel(1).0,
        };
        let mut loom = changes.watch("project_loom");

        changes.tell("project_other");
        changes.tell("project_other");

        assert!(
            heard(&mut loom).await,
            "what it missed may have been its own project, so it asks again"
        );
    }

    #[tokio::test]
    async fn a_still_watch_or_one_whose_changes_went_away_hears_nothing() {
        let changes = AccessChanges::new();
        let mut gone = changes.watch("project_loom");
        drop(changes);

        assert!(!heard(&mut AccessWatch::still()).await);
        assert!(!heard(&mut gone).await);
    }
}
