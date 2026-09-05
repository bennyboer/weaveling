use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use web_sys::Storage;

const REMEMBERED: &str = "weaveling.theme";
const MARKED: &str = "data-theme";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    System,
    Dark,
}

impl Theme {
    const EVERY: [Self; 3] = [Self::Light, Self::System, Self::Dark];

    fn stored(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::System => "system",
            Self::Dark => "dark",
        }
    }

    fn told(said: &str) -> Self {
        match said {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    fn named(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::System => "Follow the system",
            Self::Dark => "Dark",
        }
    }
}

pub fn picker() -> impl IntoView {
    let chosen = RwSignal::new(remembered());

    html::div()
        .class("theming")
        .attr("role", "group")
        .attr("aria-label", "Theme")
        .child(
            Theme::EVERY
                .into_iter()
                .map(|theme| swatch(theme, chosen))
                .collect::<Vec<_>>(),
        )
}

fn swatch(theme: Theme, chosen: RwSignal<Theme>) -> impl IntoView {
    html::button()
        .r#type("button")
        .attr("aria-label", theme.named())
        .attr("aria-pressed", move || {
            if chosen.get() == theme {
                "true"
            } else {
                "false"
            }
        })
        .on(ev::click, move |_| {
            choose(theme);
            chosen.set(theme);
        })
        .child(mark(theme))
}

fn choose(theme: Theme) {
    if let Some(root) = document().document_element() {
        let _ = match theme {
            Theme::System => root.remove_attribute(MARKED),
            chosen => root.set_attribute(MARKED, chosen.stored()),
        };
    }

    if let Some(shelf) = shelf() {
        let _ = shelf.set_item(REMEMBERED, theme.stored());
    }
}

fn remembered() -> Theme {
    shelf()
        .and_then(|shelf| shelf.get_item(REMEMBERED).ok().flatten())
        .map_or(Theme::System, |said| Theme::told(&said))
}

fn shelf() -> Option<Storage> {
    window().local_storage().ok().flatten()
}

fn mark(theme: Theme) -> impl IntoView {
    match theme {
        Theme::Light => view! {
            <svg
                width="15"
                height="15"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                stroke-width="1.3"
                stroke-linecap="round"
                aria-hidden="true"
            >
                <circle cx="8" cy="8" r="3.1"></circle>
                <path d="M8 1.1v1.5M8 13.4v1.5M1.1 8h1.5M13.4 8h1.5M3.2 3.2l1.1 1.1M11.7 11.7l1.1 1.1M12.8 3.2l-1.1 1.1M4.3 11.7l-1.1 1.1"></path>
            </svg>
        }
        .into_any(),
        Theme::System => view! {
            <svg
                width="15"
                height="15"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                stroke-width="1.3"
                aria-hidden="true"
            >
                <circle cx="8" cy="8" r="5.4"></circle>
                <path d="M8 2.6a5.4 5.4 0 0 1 0 10.8z" fill="currentColor" stroke="none"></path>
            </svg>
        }
        .into_any(),
        Theme::Dark => view! {
            <svg
                width="15"
                height="15"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                stroke-width="1.3"
                stroke-linejoin="round"
                aria-hidden="true"
            >
                <path d="M13.1 9.5A5.6 5.6 0 0 1 6.5 2.9 5.6 5.6 0 1 0 13.1 9.5z"></path>
            </svg>
        }
        .into_any(),
    }
}
