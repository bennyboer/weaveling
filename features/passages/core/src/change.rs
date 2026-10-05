use crate::{IdeaLink, PassageId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassageChange {
    IdeaLinked { passage: PassageId, idea: IdeaLink },
    IdeaUnlinked { passage: PassageId, idea: IdeaLink },
    Deleted { passage: PassageId },
}

impl PassageChange {
    pub fn passage(&self) -> PassageId {
        match self {
            Self::IdeaLinked { passage, .. }
            | Self::IdeaUnlinked { passage, .. }
            | Self::Deleted { passage } => *passage,
        }
    }
}
