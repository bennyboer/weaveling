use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, view};
use leptos_router::components::A;

use crate::route;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Viewing {
    Board,
    Pieces,
}

impl Viewing {
    fn named(&self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::Pieces => "Pieces",
        }
    }

    fn at(&self, project: &str) -> String {
        match self {
            Self::Board => route::board(project),
            Self::Pieces => route::pool(project),
        }
    }
}

pub fn mark() -> impl IntoView {
    view! {
        <svg
            width="26"
            height="26"
            viewBox="0 0 32 32"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
            aria-hidden="true"
        >
            <circle cx="16" cy="16" r="13" stroke="var(--accent)" stroke-dasharray="3 5"></circle>
            <path d="M6 11c7 6 13 6 20 0M5 20c8-5 14-5 22 0" opacity="0.45"></path>
            <ellipse cx="16" cy="17" rx="5" ry="6"></ellipse>
            <circle cx="13.6" cy="14" r="1.7" fill="currentColor"></circle>
            <circle cx="18.4" cy="14" r="1.7" fill="currentColor"></circle>
            <path d="M11 15l-4-3M21 15l4-3M11 19l-4 2M21 19l4 2"></path>
        </svg>
    }
}

pub fn masthead(project: String, named: String, here: Option<Viewing>) -> impl IntoView {
    html::header().class("masthead").child((
        view! {
            <A href=route::WORKSPACE attr:class="wordmark" attr:aria-label="All projects">
                {mark()}
                <span>"Weaveling"</span>
            </A>
        },
        html::h1().class("whose").child(html::span().child(named)),
        html::nav()
            .class("views")
            .attr("aria-label", "Views")
            .child(
                [Viewing::Board, Viewing::Pieces]
                    .into_iter()
                    .map(|view| tab(view, project.clone(), here))
                    .collect::<Vec<_>>(),
            ),
    ))
}

fn tab(view: Viewing, project: String, here: Option<Viewing>) -> impl IntoView {
    let at = view.at(&project);
    let here = here == Some(view);

    view! {
        <A href=at attr:class=move || if here { "view here" } else { "view" }>
            <span>{view.named()}</span>
        </A>
    }
}
