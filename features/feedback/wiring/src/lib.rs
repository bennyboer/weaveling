use std::sync::Arc;

use feedback_core::FeedbackCore;
use feedback_messaging::RememberCopiesListener;
use outbox::Outbox;
use wiring::{Context, Feature, Wired};

// TODO: the event store, the outbox and the catalogs, once feedback is kept (step 5)
pub struct Ports;

pub fn wire(_ports: &Ports, context: &Context) -> Wired {
    let feedback = FeedbackCore::new(context.clock.clone());

    Wired::serving(feedback_rest::router(feedback.clone()))
        .listening(vec![Arc::new(RememberCopiesListener::new(feedback))])
}

pub struct FeedbackFeature;

impl Feature for FeedbackFeature {
    const NAME: &'static str = "feedback";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(_pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports)
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(_pool: sqlx::SqlitePool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports)
    }

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
