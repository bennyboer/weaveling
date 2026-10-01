mod attaching;
mod cataloguing;
mod publishing;
mod sweeping;

pub use attaching::AttachedPassagesProjector;
pub use cataloguing::OutlineCatalogProjector;
pub use publishing::{
    OutlineEventPublisher, UnreadableOutlineEvent, event_in, every_event, message_for, outline_in,
    when_attached, when_detached, when_section_removed, when_started,
};

pub use sweeping::{DiscardOutlinesOnProjectDeleted, when_project_deleted};
