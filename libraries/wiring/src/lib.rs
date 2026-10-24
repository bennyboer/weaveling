mod event_sourced;
mod feature;
mod unprepared;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use event_sourced::EventSourced;
pub use feature::{Assembled, Feature, Storage, assemble, ports};
pub use unprepared::Unprepared;

#[cfg(feature = "postgres")]
pub use postgres::{Databases, ServerDatabases};

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use chores::Chore;
use clock::Clock;
use messaging::{Listener, Publisher};

pub struct Context {
    pub clock: Arc<dyn Clock>,
    pub publisher: Arc<dyn Publisher>,
}

#[async_trait]
pub trait Settling: Send + Sync {
    async fn settle(&self);
}

pub struct Wired {
    pub routes: Router,
    pub listeners: Vec<Arc<dyn Listener>>,
    pub settling: Vec<Arc<dyn Settling>>,
    pub chores: Vec<Arc<dyn Chore>>,
}

impl Wired {
    pub fn serving(routes: Router) -> Self {
        Self {
            routes,
            listeners: Vec::new(),
            settling: Vec::new(),
            chores: Vec::new(),
        }
    }

    pub fn listening(mut self, listeners: Vec<Arc<dyn Listener>>) -> Self {
        self.listeners = listeners;
        self
    }

    pub fn settling_before_stopping(mut self, settling: Vec<Arc<dyn Settling>>) -> Self {
        self.settling = settling;
        self
    }

    pub fn doing_chores(mut self, chores: Vec<Arc<dyn Chore>>) -> Self {
        self.chores = chores;
        self
    }
}
