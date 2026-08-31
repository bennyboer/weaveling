use std::sync::Arc;

use axum::Router;
use clock::Clock;
use projects_core::{ProjectService, ProjectStore};

pub struct Wired {
    pub routes: Router,
}

pub fn wire(store: Arc<dyn ProjectStore>, clock: Arc<dyn Clock>) -> Wired {
    Wired {
        routes: projects_rest::router(ProjectService::new(store, clock)),
    }
}
