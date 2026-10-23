use leptos::IntoView;
use leptos::prelude::*;

use crate::app::project_page;
use crate::boards::board::{BoardView, BoardViewProps};
use crate::shell::Viewing;

#[component]
pub fn BoardPage() -> impl IntoView {
    project_page::project_page(Viewing::Board, |project| {
        BoardView(BoardViewProps { project })
    })
}
