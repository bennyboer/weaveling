use leptos::prelude::*;

use crate::boards::carrying::Carrying;
use crate::boards::model::Placement;
use crate::boards::open_board::OpenBoard;
use crate::boards::viewport::Viewport;
use crate::pieces::model::PieceId;

#[derive(Clone, PartialEq, Eq)]
pub struct Renaming {
    pub piece: PieceId,
    pub at: Placement,
    pub was: String,
}

#[derive(Clone, Copy)]
pub struct Handles {
    pub viewport: RwSignal<Viewport>,
    pub carrying: RwSignal<Option<Carrying>>,
    pub selected: RwSignal<Option<PieceId>>,
    pub renaming: RwSignal<Option<Renaming>>,
    pub open: OpenBoard,
}
