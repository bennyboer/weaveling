use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use web_sys::{ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::drawer::{Edge, Inside, drawer};
use crate::icons::{Icon, mark};
use crate::outline::model::{Section, SectionId};
use crate::scenes::model::SceneId;
use crate::speech::{said, say, say_about};
use crate::writing::book_rail::Fold;
use crate::writing::viewport::SCENE_PLACE;
use crate::writing::walk::{Stop, Towards};
use crate::writing::write_view_state::WriteViewState;

pub const BOOK_RAIL: &str = "book-rail";

enum Entry {
    Section {
        id: SectionId,
        title: String,
        depth: usize,
        foldable: bool,
        fold: Fold,
    },
    Scene {
        id: SceneId,
        depth: usize,
    },
}

pub fn contents(state: WriteViewState) -> impl IntoView {
    let rail = state.book_rail;

    drawer(
        rail.drawer(),
        Edge::Left,
        || say("contents"),
        Inside {
            id: BOOK_RAIL,
            named: said("contents").into(),
            called: "contents",
            head: (
                html::p().class("tally").child(said("contents")),
                folding_everything(state),
            ),
            body: html::ol().class("rail-entries").child(move || {
                entries(state)
                    .into_iter()
                    .map(|entry| row(entry, state))
                    .collect::<Vec<_>>()
            }),
        },
    )
}

fn folding_everything(state: WriteViewState) -> impl IntoView {
    let rail = state.book_rail;
    let all_unfolded = move || {
        state.structure.sections().iter().all(|(_, section)| {
            let nests = nests(state, &section.id);

            match rail.fold_of(&section.id, nests) {
                Fold::Open => true,
                Fold::Outlined => section.scenes().is_empty(),
                Fold::Folded => !foldable(state, section),
            }
        })
    };

    let all_folded = move || {
        state.structure.sections().iter().all(|(_, section)| {
            !foldable(state, section)
                || rail.fold_of(&section.id, nests(state, &section.id)) == Fold::Folded
        })
    };

    (
        html::button()
            .r#type("button")
            .class("fold-all")
            .attr("aria-label", said("contents-unfold-all"))
            .attr("title", said("contents-unfold-all"))
            .disabled(all_unfolded)
            .on(ev::click, move |_| rail.unfold_all())
            .child(mark(Icon::UnfoldAll)),
        html::button()
            .r#type("button")
            .class("fold-all")
            .attr("aria-label", said("contents-fold-all"))
            .attr("title", said("contents-fold-all"))
            .disabled(all_folded)
            .on(ev::click, move |_| rail.fold_all())
            .child(mark(Icon::FoldAll)),
    )
}

fn entries(state: WriteViewState) -> Vec<Entry> {
    let rail = state.book_rail;
    let mut entries = Vec::new();
    let mut folded_below: Option<usize> = None;

    for (depth, section) in state.structure.sections() {
        if folded_below.is_some_and(|folded| depth > folded) {
            continue;
        }
        let fold = rail.fold_of(&section.id, nests(state, &section.id));
        folded_below = (fold == Fold::Folded).then_some(depth);

        entries.push(Entry::Section {
            id: section.id.clone(),
            title: section.title.clone(),
            depth,
            foldable: foldable(state, &section),
            fold,
        });
        if fold == Fold::Open {
            entries.extend(section.scenes().into_iter().map(|id| Entry::Scene {
                id,
                depth: depth + 1,
            }));
        }
    }

    entries
}

fn nests(state: WriteViewState, section: &SectionId) -> bool {
    state.structure.has_children(section)
}

fn foldable(state: WriteViewState, section: &Section) -> bool {
    !section.scenes().is_empty() || nests(state, &section.id)
}

fn row(entry: Entry, state: WriteViewState) -> impl IntoView {
    match entry {
        Entry::Section {
            id,
            title,
            depth,
            foldable,
            fold,
        } => {
            let unfolded = fold != Fold::Folded;
            let shown = match title.trim() {
                "" => say("heading-untitled"),
                _ => title,
            };
            let turning = id.clone();

            html::li()
                .class("rail-section")
                .style(("--depth", depth.to_string()))
                .child((
                    foldable.then(|| {
                        html::button()
                            .r#type("button")
                            .class("fold")
                            .attr("aria-expanded", unfolded.to_string())
                            .attr(
                                "aria-label",
                                match unfolded {
                                    true => say_about("outline-fold", "section", &shown),
                                    false => say_about("contents-unfold", "section", &shown),
                                },
                            )
                            .on(ev::click, move |_| {
                                state.book_rail.turn(turning.clone(), fold)
                            })
                            .child(match unfolded {
                                true => mark(Icon::Open),
                                false => mark(Icon::Closed),
                            })
                    }),
                    html::button()
                        .r#type("button")
                        .class("rail-entry")
                        .on(ev::click, move |_| go_to(Stop::Heading(id.clone()), state))
                        .child(shown),
                ))
                .into_any()
        }
        Entry::Scene { id, depth } => {
            let named = id.clone();

            html::li()
                .class("rail-scene")
                .style(("--depth", depth.to_string()))
                .child(
                    html::button()
                        .r#type("button")
                        .class("rail-entry")
                        .on(ev::click, move |_| go_to(Stop::Scene(id.clone()), state))
                        .child(move || {
                            let label = state.labels.of_scene(&named);

                            match label.shown.trim() {
                                "" => say("scene-an-empty-one"),
                                _ => label.shown,
                            }
                        }),
                )
                .into_any()
        }
    }
}

fn go_to(stop: Stop, state: WriteViewState) {
    if let Stop::Scene(scene) = &stop
        && let Ok(Some(place)) = document().query_selector(&format!("[{SCENE_PLACE}=\"{scene}\"]"))
    {
        let centred = ScrollIntoViewOptions::new();
        centred.set_block(ScrollLogicalPosition::Center);
        place.scroll_into_view_with_scroll_into_view_options(&centred);
    }
    if !state.walk.arrive(&stop, Towards::After) {
        state.walk.arrive_when_drawn(stop);
    }

    let narrow = window()
        .match_media("(width <= 60rem)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    if narrow {
        state.book_rail.drawer().close();
    }
}
