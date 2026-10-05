use leptos::prelude::*;
use leptos::{IntoView, view};

#[derive(Clone, Copy)]
pub enum Icon {
    Open,
    Closed,
    Grip,
    Earlier,
    Later,
    Promote,
    Demote,
    Remove,
    Passage,
    Hollow,
    Plus,
    Quill,
    Idea,
}

pub fn mark(icon: Icon) -> impl IntoView {
    let drawn = match icon {
        Icon::Open => "m6 9 6 6 6-6",
        Icon::Closed => "m9 18 6-6-6-6",
        Icon::Grip => "M9 5h.01M9 12h.01M9 19h.01M15 5h.01M15 12h.01M15 19h.01",
        Icon::Earlier => "M12 19V5M5 12l7-7 7 7",
        Icon::Later => "M12 5v14M19 12l-7 7-7-7",
        Icon::Promote => "m15 18-6-6 6-6",
        Icon::Demote => "m9 18 6-6-6-6",
        Icon::Remove => "M18 6 6 18M6 6l12 12",
        Icon::Passage => "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6",
        Icon::Hollow => "M12 8v4M12 16h.01",
        Icon::Plus => "M12 5v14M5 12h14",
        Icon::Quill => "M17.25 2.25 21.75 6.75 8.25 20.25 2.25 21.75 3.75 15.75z",
        Icon::Idea => "M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9V16h7v-2.1A6 6 0 0 0 12 3z",
    };
    let ringed = matches!(icon, Icon::Hollow);

    view! {
        <svg
            width="15"
            height="15"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            {ringed.then(|| view! { <circle cx="12" cy="12" r="9"></circle> })}
            <path d=drawn></path>
        </svg>
    }
}
