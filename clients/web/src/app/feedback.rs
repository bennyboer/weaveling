use leptos::IntoView;
use leptos::prelude::*;

use crate::app::project_page;
use crate::feedback::view;
use crate::shell::Viewing;

#[component]
pub fn FeedbackPage() -> impl IntoView {
    project_page::project_page(Viewing::Feedback, view::feedback_view)
}
