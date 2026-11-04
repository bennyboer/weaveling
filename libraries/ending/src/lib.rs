mod ends;
mod shutdown;

use std::future;
use std::sync::Arc;

use async_trait::async_trait;

pub use ends::{End, Ends};
pub use shutdown::Shutdown;

#[async_trait]
pub trait Ending: Send + Sync {
    async fn reached(&self);
}

#[derive(Clone)]
pub struct SessionEnd(Option<Arc<dyn Ending>>);

impl SessionEnd {
    pub fn of(ending: Arc<dyn Ending>) -> Self {
        Self(Some(ending))
    }

    pub fn never() -> Self {
        Self(None)
    }

    pub async fn reached(&self) {
        match &self.0 {
            Some(ending) => ending.reached().await,
            None => future::pending().await,
        }
    }
}
