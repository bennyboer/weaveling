use std::collections::HashSet;

use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement};

use crate::outline::model::{Section, SectionId};
use crate::outline::open_outline::{OpenOutline, Urge};
use crate::pieces::model::{Piece, PieceId};
use crate::route;

#[derive(Clone, PartialEq, Eq)]
enum Landing {
    Before(SectionId),
    Into(SectionId),
    After(SectionId),
}

#[derive(Clone, Copy)]
struct Held {
    carrying: RwSignal<Option<PieceId>>,
    hauling: RwSignal<Option<SectionId>>,
    landing: RwSignal<Option<Landing>>,
    over: RwSignal<Option<SectionId>>,
    editing: RwSignal<Option<SectionId>>,
    folded: RwSignal<HashSet<SectionId>>,
    open: OpenOutline,
}

#[component]
pub fn TheOutline(project: String) -> impl IntoView {
    let held = Held {
        carrying: RwSignal::new(None),
        hauling: RwSignal::new(None),
        landing: RwSignal::new(None),
        over: RwSignal::new(None),
        editing: RwSignal::new(None),
        folded: RwSignal::new(HashSet::new()),
        open: OpenOutline::open(&route::project_id(&project)),
    };
    let open = held.open;

    html::section().class("outline").child((
        move || {
            open.problem().map(|failure| {
                html::p().class("problem").role("alert").child((
                    failure.to_string(),
                    html::button()
                        .r#type("button")
                        .class("dismiss")
                        .attr("aria-label", "Dismiss")
                        .on(ev::click, move |_| open.dismiss())
                        .child("\u{00d7}"),
                ))
            })
        },
        html::div().class("laid-out").child((
            html::div().class("manuscript").child((
                html::p().class("tally").child("Manuscript"),
                html::ul()
                    .class("branches")
                    .attr("aria-label", "The manuscript")
                    .child(move || twigs(None, held)),
                move || {
                    (open.ready() && open.sections().is_empty()).then(|| {
                        html::p()
                            .class("empty")
                            .child("Nothing in the book yet. Add a section to begin.")
                    })
                },
                html::button()
                    .r#type("button")
                    .class("begin")
                    .on(ev::click, move |_| {
                        open.add(None, last_top(open), String::new())
                    })
                    .child((mark(Icon::Plus), "Add a section")),
            )),
            rail(held),
        )),
    ))
}

fn last_top(open: OpenOutline) -> Option<SectionId> {
    open.sections()
        .iter()
        .rfind(|held| held.parent.is_none())
        .map(|held| held.id.clone())
}

fn twigs(parent: Option<SectionId>, held: Held) -> AnyView {
    let open = held.open;

    view! {
        <For
            each=move || {
                let parent = parent.clone();
                open.sections()
                    .into_iter()
                    .filter(move |section| section.parent == parent)
                    .collect::<Vec<_>>()
            }
            key=|section| section.id.clone()
            let:section
        >
            {branch(section, held)}
        </For>
    }
    .into_any()
}

fn branch(section: Section, held: Held) -> AnyView {
    let open = held.open;
    let id = section.id.clone();
    let borne = id.clone();
    let folding = id.clone();
    let under = id.clone();

    html::li()
        .class("branch")
        .class(("last", move || last_of(&borne, held)))
        .child((row(section, held), move || {
            let shut = held.folded.with(|shut| shut.contains(&folding));
            let pieces = open.pieces_in(&under);

            (bears(&under, held) && !shut).then(|| {
                html::ul().class("twigs").child((
                    pieces
                        .iter()
                        .map(|piece| leaf(piece.clone(), open))
                        .collect::<Vec<_>>(),
                    twigs(Some(under.clone()), held),
                ))
            })
        }))
        .into_any()
}

fn bears(section: &SectionId, held: Held) -> bool {
    !held.open.pieces_in(section).is_empty() || !twigs_under(section, held).is_empty()
}

fn row(section: Section, held: Held) -> impl IntoView {
    let open = held.open;
    let id = section.id.clone();
    let folding = id.clone();
    let shutting = id.clone();
    let named = id.clone();
    let urged = id.clone();
    let pruned = id.clone();
    let landing = id.clone();
    let leaving = id.clone();
    let arriving = id.clone();
    let shuffled = id.clone();
    let folded_named = id.clone();
    let labelled = id.clone();
    let valued = id.clone();
    let toolbarred = id.clone();
    let hollowed = id.clone();
    let field = NodeRef::<html::Input>::new();
    let bearing = id.clone();

    Effect::new(move |_| {
        let wanted = held.open.just_added().or_else(|| held.editing.get());

        if wanted.as_ref() == Some(&arriving)
            && let Some(field) = field.get()
        {
            let elsewhere = document().active_element().is_none_or(|already| {
                !already.is_same_node(Some(AsRef::<web_sys::Node>::as_ref(&field)))
            });

            if elsewhere {
                let _ = field.focus();

                if held.open.just_added().is_some() {
                    field.select();
                    held.open.settled_in();
                }
            }
        }
    });

    html::div()
        .class("row")
        .class(("landing", {
            let mine = landing.clone();

            move || held.over.with(|over| over.as_ref() == Some(&mine))
        }))
        .class(("nesting", {
            let mine = landing.clone();

            move || {
                held.landing
                    .with(|at| at.as_ref() == Some(&Landing::Into(mine.clone())))
            }
        }))
        .class(("above", {
            let mine = landing.clone();

            move || {
                held.landing
                    .with(|at| at.as_ref() == Some(&Landing::Before(mine.clone())))
            }
        }))
        .class(("below", {
            let mine = landing.clone();

            move || {
                held.landing
                    .with(|at| at.as_ref() == Some(&Landing::After(mine.clone())))
            }
        }))
        .class(("hauled", {
            let mine = landing.clone();

            move || held.hauling.with(|borne| borne.as_ref() == Some(&mine))
        }))
        .attr("data-section", id.to_string())
        .on(ev::click, move |_| {
            let Some(piece) = held.carrying.get_untracked() else {
                return;
            };

            open.attach(piece, leaving.clone());
            held.carrying.set(None);
            held.over.set(None);
        })
        .child((
            grip(id.clone(), held),
            move || {
                if !bears(&bearing, held) {
                    return html::span().class("stub").into_any();
                }

                let named = folded_named.clone();
                let told = shutting.clone();
                let turned = folding.clone();
                let drawn = id.clone();

                html::button()
                    .r#type("button")
                    .class("fold")
                    .attr("aria-label", move || {
                        format!("Fold {}", shown_or_blank(&open.title_of(&named)))
                    })
                    .attr("aria-expanded", move || {
                        (!held.folded.with(|shut| shut.contains(&told))).to_string()
                    })
                    .on(ev::click, move |event| {
                        event.stop_propagation();
                        held.folded.update(|shut| {
                            if !shut.remove(&turned) {
                                shut.insert(turned.clone());
                            }
                        });
                    })
                    .child(
                        move || match held.folded.with(|shut| shut.contains(&drawn)) {
                            true => mark(Icon::Closed),
                            false => mark(Icon::Open),
                        },
                    )
                    .into_any()
            },
            html::input()
                .r#type("text")
                .class("title")
                .attr("aria-label", {
                    let mine = labelled.clone();

                    move || format!("Section {}", shown_or_blank(&open.title_of(&mine)))
                })
                .placeholder("Untitled")
                .prop("value", {
                    let mine = valued.clone();

                    move || open.title_of(&mine)
                })
                .node_ref(field)
                .on(ev::focusin, {
                    let mine = named.clone();

                    move |_| held.editing.set(Some(mine.clone()))
                })
                .on(ev::keydown, move |event| match event.key().as_str() {
                    "Enter" => {
                        event.prevent_default();
                        settle(&event, &named, open);
                        open.add(
                            open.sections()
                                .iter()
                                .find(|held| held.id == named)
                                .and_then(|held| held.parent.clone()),
                            Some(named.clone()),
                            String::new(),
                        );
                    }
                    "Tab" => {
                        event.prevent_default();
                        settle(&event, &urged, open);
                        open.urge(
                            urged.clone(),
                            match event.shift_key() {
                                true => Urge::Promote,
                                false => Urge::Demote,
                            },
                        );
                    }
                    "ArrowUp" if event.alt_key() => {
                        event.prevent_default();
                        open.urge(shuffled.clone(), Urge::Earlier);
                    }
                    "ArrowDown" if event.alt_key() => {
                        event.prevent_default();
                        open.urge(shuffled.clone(), Urge::Later);
                    }
                    "Escape" => {
                        held.editing.set(None);

                        if let Some(field) = typed(&event) {
                            let _ = field.blur();
                        }
                    }
                    _ => {}
                })
                .on(ev::focusout, {
                    let mine = pruned.clone();

                    move |event| settle(&event, &mine, open)
                }),
            move || {
                (!bears(&hollowed, held)).then(|| {
                    html::span()
                        .class("hollow")
                        .child((mark(Icon::Hollow), "empty"))
                })
            },
            html::div()
                .class("row-actions")
                .attr("role", "toolbar")
                .attr("aria-label", {
                    let mine = toolbarred.clone();

                    move || format!("Actions for {}", shown_or_blank(&open.title_of(&mine)))
                })
                .child((
                    urging(Urge::Earlier, pruned.clone(), open),
                    urging(Urge::Later, pruned.clone(), open),
                    urging(Urge::Promote, pruned.clone(), open),
                    urging(Urge::Demote, pruned.clone(), open),
                    pruning(pruned.clone(), open),
                )),
        ))
}

fn grip(section: SectionId, held: Held) -> impl IntoView {
    let open = held.open;
    let labelled = section.clone();
    let hauled = section.clone();

    html::button()
        .r#type("button")
        .class("grip")
        .attr("aria-label", move || {
            format!("Move {}", shown_or_blank(&open.title_of(&labelled)))
        })
        .on(ev::pointerdown, move |event| {
            event.stop_propagation();

            if let Some(under) = event
                .current_target()
                .and_then(|it| it.dyn_into::<HtmlElement>().ok())
            {
                let _ = under.set_pointer_capture(event.pointer_id());
            }

            held.hauling.set(Some(hauled.clone()));
            held.landing.set(None);
        })
        .on(ev::pointermove, move |event| {
            let Some(borne) = held.hauling.get_untracked() else {
                return;
            };

            held.landing
                .set(landing_at(event.client_x(), event.client_y(), &borne, held));
        })
        .on(ev::pointerup, move |_| {
            let borne = held.hauling.get_untracked();
            let at = held.landing.get_untracked();
            held.hauling.set(None);
            held.landing.set(None);

            let (Some(borne), Some(at)) = (borne, at) else {
                return;
            };

            let (under, after) = match at {
                Landing::Into(target) => {
                    let last = open.landing_into(&target);
                    (Some(target), last)
                }
                Landing::Before(target) => (open.parent_of(&target), open.landing_before(&target)),
                Landing::After(target) => (open.parent_of(&target), Some(target)),
            };

            open.place(borne, under, after);
        })
        .on(ev::pointercancel, move |_| {
            held.hauling.set(None);
            held.landing.set(None);
        })
        .child(mark(Icon::Grip))
}

fn landing_at(x: i32, y: i32, borne: &SectionId, held: Held) -> Option<Landing> {
    let row = document()
        .element_from_point(x as f32, y as f32)?
        .closest("[data-section]")
        .ok()
        .flatten()?;
    let target = SectionId::from(row.get_attribute("data-section")?);

    if held.open.would_swallow(borne, &target) {
        return None;
    }

    let edge = row.get_bounding_client_rect();
    let into = (f64::from(y) - edge.top()) / edge.height();

    Some(match into {
        near if near < 0.28 => Landing::Before(target),
        near if near > 0.72 => Landing::After(target),
        _ => Landing::Into(target),
    })
}

fn leaf(piece: PieceId, open: OpenOutline) -> impl IntoView {
    let shown = open.named(&piece);
    let taken = piece.clone();

    html::li()
        .class("leaf")
        .child(html::div().class("row").child((
            mark(Icon::Written),
            html::span().class("name").child(shown.clone()),
            deed(
                format!("Take {shown} out of the book"),
                Icon::Remove,
                move || open.detach(taken.clone()),
            ),
        )))
}

fn rail(held: Held) -> impl IntoView {
    let open = held.open;

    html::aside().class("unplaced").child((
        html::p()
            .class("tally")
            .child(move || format!("Not in the book \u{00b7} {}", open.unplaced().len())),
        html::ul()
            .class("waiting")
            .attr("aria-label", "Pieces not in the book")
            .child(move || {
                open.unplaced()
                    .into_iter()
                    .map(|piece| carried(piece, held))
                    .collect::<Vec<_>>()
            }),
        move || {
            (open.ready() && open.unplaced().is_empty()).then(|| {
                html::p()
                    .class("empty")
                    .child("Every piece has a place in the book.")
            })
        },
        html::p()
            .class("how")
            .child("Drag a piece onto a section, or click it and then click where it goes."),
    ))
}

fn carried(piece: Piece, held: Held) -> impl IntoView {
    let shown = piece.shown_as().to_owned();
    let id = piece.id;
    let mine = id.clone();
    let taken = id.clone();
    let dropped = id.clone();

    html::li().child(
        html::button()
            .r#type("button")
            .class(("carrying", move || {
                held.carrying.with(|borne| borne.as_ref() == Some(&mine))
            }))
            .attr("aria-label", format!("Place {shown}"))
            .on(ev::pointerdown, move |event| {
                if let Some(under) = event
                    .current_target()
                    .and_then(|it| it.dyn_into::<HtmlElement>().ok())
                {
                    let _ = under.set_pointer_capture(event.pointer_id());
                }

                held.carrying.set(Some(taken.clone()));
                held.over.set(None);
            })
            .on(ev::pointermove, move |event| {
                if held.carrying.with_untracked(Option::is_some) {
                    held.over
                        .set(section_under(event.client_x(), event.client_y()));
                }
            })
            .on(ev::pointerup, move |_| {
                let Some(landing) = held.over.get_untracked() else {
                    return;
                };

                held.open.attach(dropped.clone(), landing);
                held.carrying.set(None);
                held.over.set(None);
            })
            .child(shown),
    )
}

fn section_under(x: i32, y: i32) -> Option<SectionId> {
    document()
        .element_from_point(x as f32, y as f32)?
        .closest("[data-section]")
        .ok()
        .flatten()
        .and_then(|row| row.get_attribute("data-section"))
        .map(SectionId::from)
}

fn twigs_under(section: &SectionId, held: Held) -> Vec<Section> {
    held.open
        .sections()
        .into_iter()
        .filter(|held| held.parent.as_ref() == Some(section))
        .collect()
}

fn last_of(section: &SectionId, held: Held) -> bool {
    let sections = held.open.sections();
    let parent = sections
        .iter()
        .find(|held| &held.id == section)
        .and_then(|held| held.parent.clone());

    sections
        .iter()
        .rfind(|held| held.parent == parent)
        .map(|held| &held.id)
        == Some(section)
}

fn urging(urge: Urge, section: SectionId, open: OpenOutline) -> impl IntoView {
    let (named, icon) = match urge {
        Urge::Earlier => ("Move earlier", Icon::Earlier),
        Urge::Later => ("Move later", Icon::Later),
        Urge::Promote => ("Promote", Icon::Promote),
        Urge::Demote => ("Demote", Icon::Demote),
    };
    let asked = section.clone();
    let labelled = section.clone();

    html::button()
        .r#type("button")
        .attr("aria-label", move || {
            format!("{named} {}", shown_or_blank(&open.title_of(&labelled)))
        })
        .prop("disabled", move || !open.can(&asked, urge))
        .on(ev::click, move |event| {
            event.stop_propagation();
            open.urge(section.clone(), urge);
        })
        .child(mark(icon))
}

fn pruning(section: SectionId, open: OpenOutline) -> impl IntoView {
    let labelled = section.clone();

    html::button()
        .r#type("button")
        .attr("aria-label", move || {
            format!("Remove {}", shown_or_blank(&open.title_of(&labelled)))
        })
        .on(ev::click, move |event| {
            event.stop_propagation();
            open.remove(section.clone());
        })
        .child(mark(Icon::Remove))
}

fn deed(what: String, icon: Icon, done: impl Fn() + 'static) -> impl IntoView {
    html::button()
        .r#type("button")
        .attr("aria-label", what)
        .on(ev::click, move |event| {
            event.stop_propagation();
            done();
        })
        .child(mark(icon))
}

fn shown_or_blank(shown: &str) -> String {
    match shown.is_empty() {
        true => "Untitled".to_owned(),
        false => shown.to_owned(),
    }
}

fn typed(event: &ev::Event) -> Option<HtmlInputElement> {
    event
        .target()
        .and_then(|it| it.dyn_into::<HtmlInputElement>().ok())
}

fn settle(event: &ev::Event, section: &SectionId, open: OpenOutline) {
    let Some(field) = typed(event) else {
        return;
    };
    let written = field.value();
    let was = open
        .sections()
        .into_iter()
        .find(|held| &held.id == section)
        .map(|held| held.title);

    if was.as_deref() != Some(written.as_str()) {
        open.retitle(section.clone(), written);
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Open,
    Closed,
    Grip,
    Earlier,
    Later,
    Promote,
    Demote,
    Remove,
    Written,
    Hollow,
    Plus,
}

fn mark(icon: Icon) -> impl IntoView {
    let drawn = match icon {
        Icon::Open => "m6 9 6 6 6-6",
        Icon::Closed => "m9 18 6-6-6-6",
        Icon::Grip => "M9 5h.01M9 12h.01M9 19h.01M15 5h.01M15 12h.01M15 19h.01",
        Icon::Earlier => "M12 19V5M5 12l7-7 7 7",
        Icon::Later => "M12 5v14M19 12l-7 7-7-7",
        Icon::Promote => "m15 18-6-6 6-6",
        Icon::Demote => "m9 18 6-6-6-6",
        Icon::Remove => "M18 6 6 18M6 6l12 12",
        Icon::Written => "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6",
        Icon::Hollow => "M12 8v4M12 16h.01",
        Icon::Plus => "M12 5v14M5 12h14",
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
