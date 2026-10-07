use messaging::Message;

use crate::event::Recorded;

pub type MessageMapping<E> = fn(&Recorded<E>) -> Option<Message>;
