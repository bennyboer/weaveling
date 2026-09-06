use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, view};
use leptos_router::components::A;

use crate::route;
use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Viewing {
    Board,
    Outline,
    Pieces,
}

impl Viewing {
    fn named(&self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::Outline => "Outline",
            Self::Pieces => "Pieces",
        }
    }

    fn at(&self, project: &str) -> String {
        match self {
            Self::Board => route::board(project),
            Self::Outline => route::outline(project),
            Self::Pieces => route::pool(project),
        }
    }
}

pub struct Inside {
    project: String,
    named: String,
    here: Option<Viewing>,
}

impl Inside {
    pub fn of(project: &str, here: Option<Viewing>) -> Self {
        Self {
            project: project.to_owned(),
            named: route::named(project),
            here,
        }
    }
}

pub fn mark() -> impl IntoView {
    html::span().class("mark").attr("aria-hidden", "true")
}

pub fn masthead(inside: Option<Inside>) -> impl IntoView {
    html::header().class("masthead").child((
        view! {
            <A href=route::WORKSPACE attr:class="wordmark" attr:aria-label="All projects">
                {mark()}
                <span>"Weaveling"</span>
            </A>
        },
        inside.map(whereabouts),
        theme::picker(),
    ))
}

fn whereabouts(inside: Inside) -> impl IntoView {
    let Inside {
        project,
        named,
        here,
    } = inside;

    (
        html::h1().class("whose").child(html::span().child(named)),
        html::nav()
            .class("views")
            .attr("aria-label", "Views")
            .child(
                [Viewing::Board, Viewing::Outline, Viewing::Pieces]
                    .into_iter()
                    .map(|view| tab(view, project.clone(), here))
                    .collect::<Vec<_>>(),
            ),
    )
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
