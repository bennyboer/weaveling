use leptos::prelude::*;

use crate::boards::carrying::Carrying;
use crate::boards::model::Placement;
use crate::boards::open_board::OpenBoard;
use crate::boards::viewport::Viewport;
use crate::pieces::model::PieceId;

#[derive(Clone, PartialEq, Eq)]
pub enum Naming {
    Capturing {
        at: Placement,
    },
    Renaming {
        piece: PieceId,
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
            Self::Capturing { .. } => "What is the idea?".to_owned(),
            Self::Renaming { was, .. } => format!("Rename {was}"),
        }
    }

    fn piece(&self) -> Option<&PieceId> {
        match self {
            Self::Capturing { .. } => None,
            Self::Renaming { piece, .. } => Some(piece),
        }
    }

    pub fn is(&self, other: &Self) -> bool {
        self.piece() == other.piece()
    }
}

#[derive(Clone, Copy)]
pub struct Handles {
    pub viewport: RwSignal<Viewport>,
    pub carrying: RwSignal<Option<Carrying>>,
    pub selected: RwSignal<Option<PieceId>>,
    pub naming: RwSignal<Option<Naming>>,
    pub open: OpenBoard,
}
