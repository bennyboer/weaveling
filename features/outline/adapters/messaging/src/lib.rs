mod attaching;
mod cataloguing;
mod discarding;
mod letting_go;
mod publishing;
mod scene_deleting;
mod sweeping;

pub use attaching::AttachmentIndexProjector;
pub use cataloguing::OutlineCatalogProjector;
pub use discarding::DetachOnDiscard;
pub use publishing::{UnreadableOutlineEvent, event_in, message_for, outline_in};
pub use scene_deleting::DetachOnSceneDeleted;

pub use sweeping::DiscardOutlinesOnProjectDeleted;
