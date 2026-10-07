#[cfg(feature = "postgres")]
pub mod database;
mod feature;
mod unprepared;

#[cfg(feature = "postgres")]
pub use database::{Databases, ServerDatabases};
pub use feature::{Assembled, Feature, Storage, assemble};
pub use unprepared::Unprepared;

use std::sync::Arc;

use axum::Router;
use clock::Clock;
use messaging::{Listener, Publisher};

pub struct Context {
    pub clock: Arc<dyn Clock>,
    pub publisher: Arc<dyn Publisher>,
}

pub struct Wired {
    pub routes: Router,
    pub listeners: Vec<Arc<dyn Listener>>,
}

impl Wired {
    pub fn serving(routes: Router) -> Self {
        Self {
            routes,
            listeners: Vec::new(),
        }
    }

    pub fn listening(mut self, listeners: Vec<Arc<dyn Listener>>) -> Self {
        self.listeners = listeners;
        self
    }
}
