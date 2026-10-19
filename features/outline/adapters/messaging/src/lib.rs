mod attaching;
mod cataloguing;
mod discarding;
mod letting_go;
mod publishing;
mod scene_deleting;
mod sweeping;

pub use attaching::IndexAttachmentsListener;
pub use cataloguing::CatalogueOutlinesListener;
pub use discarding::DetachOnDiscardListener;
pub use publishing::{UnreadableOutlineEvent, event_in, message_for, outline_in};
pub use scene_deleting::DetachOnSceneDeletedListener;

pub use sweeping::DiscardOutlinesOnProjectDeletedListener;
