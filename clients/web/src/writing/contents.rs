use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use web_sys::{ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::drawer;
use crate::drawer::{Edge, Inside};
use crate::feedback::pending::PendingHere;
use crate::icons;
use crate::icons::Icon;
use crate::outline::model::{Section, SectionId};
use crate::scenes::model::SceneId;
use crate::speech;
use crate::writing::book_rail::Fold;
use crate::writing::viewport::SCENE_PLACE;
use crate::writing::walk::{Stop, Towards};
use crate::writing::write_view_state::WriteViewState;

pub const BOOK_RAIL: &str = "book-rail";

/// A line in the contents: a section or a scene, and how many comments on it are not done yet,
/// a section's counting its scenes too, and while folded the sections under it.
enum Entry {
    Section {
        id: SectionId,
        title: String,
        depth: usize,
        foldable: bool,
        fold: Fold,
        not_done: u32,
    },
    Scene {
        id: SceneId,
        depth: usize,
        not_done: u32,
    },
}

pub fn contents(state: WriteViewState) -> impl IntoView {
    let rail = state.book_rail;
    let not_done = use_context::<PendingHere>();

    drawer::drawer(
        rail.drawer(),
        Edge::Left,
        || speech::say("contents"),
        Inside {
            id: BOOK_RAIL,
            named: speech::said("contents").into(),
            called: "contents",
            head: (
                html::p().class("tally").child(speech::said("contents")),
                folding_everything(state),
            ),
            body: html::ol().class("rail-entries").child(move || {
                entries(state, not_done)
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
            .attr("aria-label", speech::said("contents-unfold-all"))
            .attr("title", speech::said("contents-unfold-all"))
            .disabled(all_unfolded)
            .on(ev::click, move |_| rail.unfold_all())
            .child(icons::mark(Icon::UnfoldAll)),
        html::button()
            .r#type("button")
            .class("fold-all")
            .attr("aria-label", speech::said("contents-fold-all"))
            .attr("title", speech::said("contents-fold-all"))
            .disabled(all_folded)
            .on(ev::click, move |_| rail.fold_all())
            .child(icons::mark(Icon::FoldAll)),
    )
}

fn entries(state: WriteViewState, not_done: Option<PendingHere>) -> Vec<Entry> {
    let rail = state.book_rail;
    let on_scene = |scene: &SceneId| not_done.map_or(0, |not_done| not_done.on_scene(scene));
    let mut entries = Vec::new();
    let mut folded_below: Option<(usize, usize)> = None;

    for (depth, section) in state.structure.sections() {
        let scenes = section.scenes();
        let in_it = not_done.map_or(0, |not_done| not_done.on_section(&section.id))
            + scenes.iter().map(on_scene).sum::<u32>();
        if let Some((_, at)) = folded_below.filter(|(folded, _)| depth > *folded) {
            if let Some(Entry::Section { not_done, .. }) = entries.get_mut(at) {
                *not_done += in_it;
            }
            continue;
        }
        let fold = rail.fold_of(&section.id, nests(state, &section.id));
        folded_below = (fold == Fold::Folded).then_some((depth, entries.len()));

        entries.push(Entry::Section {
            id: section.id.clone(),
            title: section.title.clone(),
            depth,
            foldable: foldable(state, &section),
            fold,
            not_done: in_it,
        });
        if fold == Fold::Open {
            entries.extend(scenes.into_iter().map(|id| Entry::Scene {
                not_done: on_scene(&id),
                id,
                depth: depth + 1,
            }));
        }
    }

    entries
}

/// How many comments not done yet lie in an entry, beside its name, where there are any.
fn not_done_count(not_done: u32) -> impl IntoView {
    (not_done > 0).then(|| {
        let mut args = FluentArgs::new();
        args.set("count", not_done);
        let said = speech::say_with("contents-not-done", &args);

        html::span()
            .class("not-done")
            .attr("aria-label", said.clone())
            .attr("title", said)
            .child(not_done.to_string())
    })
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
            not_done,
        } => {
            let unfolded = fold != Fold::Folded;
            let shown = match title.trim() {
                "" => speech::say("heading-untitled"),
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
                                    true => speech::say_about("outline-fold", "section", &shown),
                                    false => {
                                        speech::say_about("contents-unfold", "section", &shown)
                                    }
                                },
                            )
                            .on(ev::click, move |_| {
                                state.book_rail.turn(turning.clone(), fold)
                            })
                            .child(match unfolded {
                                true => icons::mark(Icon::Open),
                                false => icons::mark(Icon::Closed),
                            })
                    }),
                    html::button()
                        .r#type("button")
                        .class("rail-entry")
                        .on(ev::click, move |_| go_to(Stop::Heading(id.clone()), state))
                        .child((
                            html::span().class("title").child(shown),
                            not_done_count(not_done),
                        )),
                ))
                .into_any()
        }
        Entry::Scene {
            id,
            depth,
            not_done,
        } => {
            let named = id.clone();

            html::li()
                .class("rail-scene")
                .style(("--depth", depth.to_string()))
                .child(
                    html::button()
                        .r#type("button")
                        .class("rail-entry")
                        .on(ev::click, move |_| go_to(Stop::Scene(id.clone()), state))
                        .child((
                            html::span().class("title").child(move || {
                                let label = state.labels.of_scene(&named);

                                match label.shown.trim() {
                                    "" => speech::say("scene-an-empty-one"),
                                    _ => label.shown,
                                }
                            }),
                            not_done_count(not_done),
                        )),
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
