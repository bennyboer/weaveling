mod attaching;
mod cataloguing;
mod discarding;
mod publishing;

pub use attaching::AttachedPiecesProjector;
pub use cataloguing::OutlineCatalogProjector;
pub use discarding::{DetachOnDiscard, when_discarded};
pub use publishing::{
    Publishing, UnreadableOutlineEvent, event_in, every_event, message_for, outline_in,
    when_attached, when_detached, when_section_removed, when_started,
};
