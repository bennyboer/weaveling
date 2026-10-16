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
    Scene,
    Hollow,
    Plus,
    Quill,
    Idea,
    Section,
    Flash,
    More,
    Saved,
    Saving,
    PullOut,
    PushIn,
    Settings,
    Cogwheel,
    UnfoldAll,
    FoldAll,
    Focus,
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
        Icon::Scene => "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6",
        Icon::Hollow => "M12 8v4M12 16h.01",
        Icon::Plus => "M12 5v14M5 12h14",
        Icon::Quill => "M17.25 2.25 21.75 6.75 8.25 20.25 2.25 21.75 3.75 15.75z",
        Icon::Idea => "M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9V16h7v-2.1A6 6 0 0 0 12 3z",
        Icon::Section => "M4 6h16M8 12h12M8 18h12",
        Icon::Flash => "M13 2 3 14h9l-1 8 10-12h-9z",
        Icon::More => "M12 5h.01M12 12h.01M12 19h.01",
        Icon::Saved => "M20 6 9 17l-5-5",
        Icon::PullOut => "m15 18-6-6 6-6",
        Icon::PushIn => "m9 18 6-6-6-6",
        Icon::Settings => "M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6",
        Icon::Focus => {
            "M3 7V5a2 2 0 0 1 2-2h2M17 3h2a2 2 0 0 1 2 2v2M21 17v2a2 2 0 0 1-2 2h-2M7 21H5a2 2 0 0 1-2-2v-2M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z"
        }
        Icon::UnfoldAll => "m7 15 5 5 5-5M7 9l5-5 5 5",
        Icon::FoldAll => "m7 20 5-5 5 5M7 4l5 5 5-5",
        Icon::Cogwheel => {
            "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2zM15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z"
        }
        Icon::Saving => {
            "M5 22h14M5 2h14M17 22v-4.17a2 2 0 0 0-.59-1.42L12 12l-4.41 4.41A2 2 0 0 0 7 17.83V22M7 2v4.17a2 2 0 0 0 .59 1.42L12 12l4.41-4.41A2 2 0 0 0 17 6.17V2"
        }
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
