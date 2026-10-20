use leptos::prelude::*;

use crate::boards::board_view_state::BoardViewState;
use crate::boards::carrying::Carrying;
use crate::boards::model::Placement;
use crate::boards::viewport::Viewport;
use crate::ideas::model::IdeaId;
use crate::speech::{say, say_about};

#[derive(Clone, PartialEq, Eq)]
pub enum Naming {
    Capturing {
        at: Placement,
    },
    Renaming {
        idea: IdeaId,
        at: Placement,
        was: String,
    },
}

impl Naming {
    pub fn at(&self) -> Placement {
        match self {
            Self::Capturing { at } => *at,
            Self::Renaming { at, .. } => *at,
        }
    }

    pub fn was(&self) -> &str {
        match self {
            Self::Capturing { .. } => "",
            Self::Renaming { was, .. } => was,
        }
    }

    pub fn asked(&self) -> String {
        match self {
            Self::Capturing { .. } => say("idea-asked"),
            Self::Renaming { was, .. } => say_about("board-rename", "idea", was),
        }
    }

    fn idea(&self) -> Option<&IdeaId> {
        match self {
            Self::Capturing { .. } => None,
            Self::Renaming { idea, .. } => Some(idea),
        }
    }

    pub fn is(&self, other: &Self) -> bool {
        self.idea() == other.idea()
    }
}

#[derive(Clone, Copy)]
pub struct Handles {
    pub viewport: RwSignal<Viewport>,
    pub carrying: RwSignal<Option<Carrying>>,
    pub selected: RwSignal<Option<IdeaId>>,
    pub naming: RwSignal<Option<Naming>>,
    pub state: BoardViewState,
}
