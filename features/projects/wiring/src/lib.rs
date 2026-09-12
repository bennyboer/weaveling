use std::sync::Arc;

use projects_core::{ProjectService, ProjectStore};
use projects_store::InMemoryProjectStore;
use wiring::{Context, Wired};

pub struct Ports {
    pub store: Arc<dyn ProjectStore>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(InMemoryProjectStore::new()),
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        Self {
            store: Arc::new(projects_store::PostgresProjectStore::new(pool)),
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let projects = ProjectService::new(ports.store.clone(), context.clock.clone());

    Wired::serving(projects_rest::router(projects))
}

pub const NAME: &str = "projects";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, projects_store::migrations()).await
}
