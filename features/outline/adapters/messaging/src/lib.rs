mod attaching;
mod cataloguing;
mod discarding;
mod letting_go;
mod publishing;
mod scene_deleting;
mod sweeping;

pub use attaching::AttachmentIndexProjector;
pub use cataloguing::OutlineCatalogProjector;
pub use discarding::{DetachOnDiscard, when_discarded};
pub use publishing::{
    UnreadableOutlineEvent, event_in, every_event, message_for, outline_in, when_attached,
    when_detached, when_section_removed, when_started,
};
pub use scene_deleting::{DetachOnSceneDeleted, when_scene_deleted};

pub use sweeping::{DiscardOutlinesOnProjectDeleted, when_project_deleted};
