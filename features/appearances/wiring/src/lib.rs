use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::AppearanceCatalog;
use appearances_messaging::{
    ForgetDiscardedIdea, OutlineAppearancesProjector, PassageAppearancesProjector,
};
use wiring::Wired;

pub struct Ports {
    pub catalog: Arc<dyn AppearanceCatalog>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            catalog: Arc::new(InMemoryAppearanceCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        Self {
            catalog: Arc::new(appearances_catalog::PostgresAppearanceCatalog::new(pool)),
        }
    }
}

pub fn wire(ports: &Ports) -> Wired {
    Wired::serving(appearances_rest::router(ports.catalog.clone())).listening(vec![
        Arc::new(OutlineAppearancesProjector::new(ports.catalog.clone())),
        Arc::new(PassageAppearancesProjector::new(ports.catalog.clone())),
        Arc::new(ForgetDiscardedIdea::new(ports.catalog.clone())),
    ])
}

pub const NAME: &str = "appearances";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, appearances_catalog::migrations()).await
}
