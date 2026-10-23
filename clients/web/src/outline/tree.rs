use std::collections::HashSet;
use std::time::Duration;

use fluent_bundle::FluentArgs;
use leptos::html::{self, Li};
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement};

use crate::dialog;
use crate::icons;
use crate::icons::Icon;
use crate::outline::in_view::{InView, LEAVES_PLACE};
use crate::outline::model::{Attachment, Section, SectionId};
use crate::outline::outline_view_state::{OutlineViewState, Urge};
use crate::problem;
use crate::route;
use crate::speech;
use crate::speech::Speech;
use crate::tray;

#[derive(Clone, PartialEq, Eq)]
enum Landing {
    Before(SectionId),
    Into(SectionId),
    After(SectionId),
}

#[derive(Clone, Copy)]
struct Held {
    project: StoredValue<String>,
    carrying: RwSignal<Option<Attachment>>,
    hauling: RwSignal<Option<SectionId>>,
    landing: RwSignal<Option<Landing>>,
    over: RwSignal<Option<SectionId>>,
    editing: RwSignal<Option<SectionId>>,
    folded: RwSignal<HashSet<SectionId>>,
    in_view: InView,
    state: OutlineViewState,
    renamed_meanwhile: RwSignal<Option<RenamedMeanwhile>>,
}

#[derive(Clone)]
struct RenamedMeanwhile {
    section: SectionId,
    mine: String,
    theirs: String,
    field: NodeRef<html::Input>,
}

const FLOCK: usize = 50;

#[component]
pub fn OutlineView(project: String) -> impl IntoView {
    let carrying = RwSignal::new(None);
    let hauling = RwSignal::new(None);
    let in_view = InView::around_the_window(Signal::derive(move || {
        carrying.with(Option::is_some) || hauling.with(Option::is_some)
    }));
    on_cleanup(move || in_view.close());
    let held = Held {
        project: StoredValue::new(project.clone()),
        carrying,
        hauling,
        landing: RwSignal::new(None),
        over: RwSignal::new(None),
        editing: RwSignal::new(None),
        folded: RwSignal::new(HashSet::new()),
        in_view,
        state: OutlineViewState::open(&route::project_id(&project)),
        renamed_meanwhile: RwSignal::new(None),
    };
    let state = held.state;

    html::section().class("outline").child((
        move || {
            state
                .problem()
                .map(|failure| problem::problem_to_dismiss(failure, move || state.dismiss()))
        },
        move || {
            held.renamed_meanwhile
                .get()
                .map(|asked| renamed_meanwhile(asked, held))
        },
        tray::laid_out(
            html::div()
                .class("manuscript")
                .child((
                    html::p()
                        .class("tally")
                        .child(speech::said("outline-manuscript")),
                    html::ul()
                        .class("branches")
                        .attr("aria-label", speech::said("outline-the-manuscript"))
                        .child(move || twigs(None, held)),
                    move || {
                        (state.ready() && state.sections().is_empty()).then(|| {
                            html::p()
                                .class("empty")
                                .child(speech::said("outline-nothing-yet"))
                        })
                    },
                    html::button()
                        .r#type("button")
                        .class("begin")
                        .prop("disabled", move || !state.ready())
                        .on(ev::click, move |_| {
                            state.add(None, last_top(state), String::new())
                        })
                        .child((icons::mark(Icon::Plus), speech::said("outline-add-section"))),
                ))
                .into_any(),
            kept(held).into_any(),
            move || {
                speech::say_about(
                    "outline-not-in-the-book",
                    "count",
                    &state.waiting_scenes().len().to_string(),
                )
            },
        ),
    ))
}

fn last_top(state: OutlineViewState) -> Option<SectionId> {
    state
        .sections()
        .iter()
        .rfind(|held| held.parent.is_none())
        .map(|held| held.id.clone())
}

fn twigs(parent: Option<SectionId>, held: Held) -> AnyView {
    let state = held.state;

    view! {
        <For
            each=move || {
                let parent = parent.clone();
                state.sections()
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
    let state = held.state;
    let id = section.id.clone();
    let folding = id.clone();
    let under = id.clone();

    html::li()
        .class("branch")
        .child((row(section, held), move || {
            let shut = held.folded.with(|shut| shut.contains(&folding));
            let (scenes, ideas): (Vec<Attachment>, Vec<Attachment>) = state
                .attachments_in(&under)
                .into_iter()
                .partition(|attachment| matches!(attachment, Attachment::Scene(_)));

            (bears(&under, held) && !shut).then(|| {
                html::ul().class("twigs").child((
                    group(under.clone(), "scenes", scenes, held),
                    group(under.clone(), "ideas", ideas, held),
                    twigs(Some(under.clone()), held),
                ))
            })
        }))
        .into_any()
}

fn group(
    section: SectionId,
    kind: &'static str,
    attachments: Vec<Attachment>,
    held: Held,
) -> Option<impl IntoView> {
    (!attachments.is_empty()).then(|| {
        html::li().class("kind").child((
            html::div()
                .class("row")
                .child(html::span().class("kind-label").child(named_as(kind))),
            html::ul()
                .class("twigs")
                .attr("aria-label", named_as(kind))
                .child(
                    attachments
                        .chunks(FLOCK)
                        .enumerate()
                        .map(|(nth, flock)| {
                            leaves(format!("{section}/{kind}/{nth}"), flock.to_vec(), held)
                        })
                        .collect::<Vec<_>>(),
                ),
        ))
    })
}

fn leaves(place: String, attachments: Vec<Attachment>, held: Held) -> impl IntoView {
    let spot: NodeRef<Li> = NodeRef::new();
    let rows = attachments.len();
    let near = held.in_view.watched(place.clone(), spot);

    html::li()
        .class("leaves")
        .class(("waiting", move || !near.get()))
        .attr(LEAVES_PLACE, place)
        .style(("--rows", rows.to_string()))
        .node_ref(spot)
        .child(move || {
            near.get().then(|| {
                html::ul().class("flock").child(
                    attachments
                        .iter()
                        .cloned()
                        .map(|attachment| leaf(attachment, held))
                        .collect::<Vec<_>>(),
                )
            })
        })
}

fn bears(section: &SectionId, held: Held) -> bool {
    !held.state.attachments_in(section).is_empty() || !twigs_under(section, held).is_empty()
}

fn row(section: Section, held: Held) -> impl IntoView {
    let state = held.state;
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
    let began_with = StoredValue::new(None::<String>);
    let bearing = id.clone();

    Effect::new(move |_| {
        let wanted = held.state.just_added().or_else(|| held.editing.get());

        if wanted.as_ref() == Some(&arriving)
            && let Some(field) = field.get()
        {
            let elsewhere = document().active_element().is_none_or(|already| {
                !already.is_same_node(Some(AsRef::<web_sys::Node>::as_ref(&field)))
            });

            if elsewhere {
                let _ = field.focus();

                if held.state.just_added().is_some() {
                    field.select();
                    held.state.settled_in();
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

            move || held.hauling.with(|section| section.as_ref() == Some(&mine))
        }))
        .attr("data-section", id.to_string())
        .on(ev::click, move |_| {
            let Some(attachment) = held.carrying.get_untracked() else {
                return;
            };

            state.attach(attachment, leaving.clone());
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
                        about_section("outline-fold", &state.title_of(&named))
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
                            true => icons::mark(Icon::Closed),
                            false => icons::mark(Icon::Open),
                        },
                    )
                    .into_any()
            },
            html::input()
                .r#type("text")
                .class("title")
                .attr("aria-label", {
                    let mine = labelled.clone();

                    move || about_section("outline-section", &state.title_of(&mine))
                })
                .placeholder(speech::said("untitled"))
                .prop("value", {
                    let mine = valued.clone();
                    let own = Memo::new(move |_| state.title_of(&mine));

                    move || {
                        let title = own.get();

                        match field
                            .get_untracked()
                            .filter(|shown| shown.matches(":focus").unwrap_or(false))
                        {
                            Some(typing) => typing.value(),
                            None => title,
                        }
                    }
                })
                .node_ref(field)
                .on(ev::focusin, {
                    let mine = named.clone();

                    move |_| {
                        began_with.set_value(Some(state.title_of(&mine)));
                        held.editing.set(Some(mine.clone()));
                    }
                })
                .on(ev::keydown, move |event| match event.key().as_str() {
                    "Enter" => {
                        event.prevent_default();
                        settle(&event, &named, held, began_with, field);
                        state.add(
                            state
                                .sections()
                                .iter()
                                .find(|held| held.id == named)
                                .and_then(|held| held.parent.clone()),
                            Some(named.clone()),
                            String::new(),
                        );
                    }
                    "Tab" => {
                        event.prevent_default();
                        settle(&event, &urged, held, began_with, field);
                        state.urge(
                            urged.clone(),
                            match event.shift_key() {
                                true => Urge::Promote,
                                false => Urge::Demote,
                            },
                        );
                    }
                    "ArrowUp" if event.alt_key() => {
                        event.prevent_default();
                        state.urge(shuffled.clone(), Urge::Earlier);
                    }
                    "ArrowDown" if event.alt_key() => {
                        event.prevent_default();
                        state.urge(shuffled.clone(), Urge::Later);
                    }
                    "Escape" => {
                        held.editing.set(None);

                        let _ = event_target::<HtmlInputElement>(&event).blur();
                    }
                    _ => {}
                })
                .on(ev::focusout, {
                    let mine = pruned.clone();

                    move |event| settle(&event, &mine, held, began_with, field)
                }),
            move || {
                (!bears(&hollowed, held)).then(|| {
                    html::span()
                        .class("hollow")
                        .child((icons::mark(Icon::Hollow), "empty"))
                })
            },
            html::div()
                .class("row-actions")
                .attr("role", "toolbar")
                .attr("aria-label", {
                    let mine = toolbarred.clone();

                    move || about_section("outline-actions", &state.title_of(&mine))
                })
                .child((
                    urging(Urge::Earlier, pruned.clone(), state),
                    urging(Urge::Later, pruned.clone(), state),
                    urging(Urge::Promote, pruned.clone(), state),
                    urging(Urge::Demote, pruned.clone(), state),
                    writing(pruned.clone(), state),
                    pruning(pruned.clone(), state),
                )),
        ))
}

fn grip(section: SectionId, held: Held) -> impl IntoView {
    let state = held.state;
    let labelled = section.clone();
    let hauled = section.clone();

    html::button()
        .r#type("button")
        .class("grip")
        .attr("aria-label", move || {
            about_section("outline-move", &state.title_of(&labelled))
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
            let Some(section) = held.hauling.get_untracked() else {
                return;
            };

            held.landing.set(landing_at(
                event.client_x(),
                event.client_y(),
                &section,
                held,
            ));
        })
        .on(ev::pointerup, move |_| {
            let section = held.hauling.get_untracked();
            let at = held.landing.get_untracked();
            held.hauling.set(None);
            held.landing.set(None);

            let (Some(section), Some(at)) = (section, at) else {
                return;
            };

            let (under, after) = match at {
                Landing::Into(target) => {
                    let last = state.landing_into(&target);
                    (Some(target), last)
                }
                Landing::Before(target) => {
                    (state.parent_of(&target), state.landing_before(&target))
                }
                Landing::After(target) => (state.parent_of(&target), Some(target)),
            };

            state.place(section, under, after);
        })
        .on(ev::pointercancel, move |_| {
            held.hauling.set(None);
            held.landing.set(None);
        })
        .child(icons::mark(Icon::Grip))
}

fn landing_at(x: i32, y: i32, section: &SectionId, held: Held) -> Option<Landing> {
    let row = document()
        .element_from_point(x as f32, y as f32)?
        .closest("[data-section]")
        .ok()
        .flatten()?;
    let target = SectionId::from(row.get_attribute("data-section")?);

    if held.state.would_swallow(section, &target) {
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

fn icon_for(attachment: &Attachment) -> Icon {
    match attachment {
        Attachment::Scene(_) => Icon::Scene,
        Attachment::Idea(_) => Icon::Idea,
    }
}

fn leaf(attachment: Attachment, held: Held) -> impl IntoView {
    let state = held.state;
    let shown = state.named(&attachment);
    let taken = attachment.clone();
    let named = shown.clone();
    let marked = icons::mark(icon_for(&attachment));
    let at = held.project.with_value(|project| match &attachment {
        Attachment::Scene(scene) => route::writing_at(project, scene),
        Attachment::Idea(idea) => route::idea(project, idea, &shown),
    });

    html::li()
        .class("leaf")
        .child(html::div().class("row").child((
            marked,
            view! {
                <A href=at attr:class="name">
                    {named}
                </A>
            },
            deed(
                speech::say_about("outline-take-out", "name", &shown),
                Icon::Remove,
                move || state.detach(taken.clone()),
            ),
        )))
}

fn kept(held: Held) -> impl IntoView {
    let state = held.state;

    (
        html::p().class("tally").child(move || {
            speech::say_about(
                "outline-not-in-the-book",
                "count",
                &state.waiting_scenes().len().to_string(),
            )
        }),
        html::ul()
            .class("waiting")
            .attr("aria-label", speech::said("outline-scenes-waiting"))
            .child(move || {
                state
                    .waiting_scenes()
                    .into_iter()
                    .map(|scene| {
                        let shown = scene.shown_as();

                        carried(Attachment::Scene(scene.id), shown, held)
                    })
                    .collect::<Vec<_>>()
            }),
        move || {
            (state.ready() && state.waiting_scenes().is_empty()).then(|| {
                html::p()
                    .class("empty")
                    .child(speech::said("outline-every-scene-placed"))
            })
        },
        html::p()
            .class("tally")
            .child(speech::said("outline-ideas")),
        html::ul()
            .class("waiting")
            .attr("aria-label", speech::said("outline-ideas-waiting"))
            .child(move || {
                state
                    .waiting_ideas()
                    .into_iter()
                    .map(|idea| {
                        let shown = idea.shown_as().to_owned();

                        carried(Attachment::Idea(idea.id), shown, held)
                    })
                    .collect::<Vec<_>>()
            }),
        html::p()
            .class("how")
            .child(speech::said("outline-how-to-place")),
    )
}

fn carried(attachment: Attachment, shown: String, held: Held) -> impl IntoView {
    let mine = attachment.clone();
    let taken = attachment.clone();
    let dropped = attachment;
    let marked = icons::mark(icon_for(&mine));

    html::li().child(
        html::button()
            .r#type("button")
            .class(("carrying", move || {
                held.carrying
                    .with(|attachment| attachment.as_ref() == Some(&mine))
            }))
            .attr("aria-label", {
                let shown = shown.clone();

                move || speech::say_about("outline-place", "name", &shown)
            })
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

                held.state.attach(dropped.clone(), landing);
                held.carrying.set(None);
                held.over.set(None);
            })
            .child((marked, shown)),
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
    held.state
        .sections()
        .into_iter()
        .filter(|held| held.parent.as_ref() == Some(section))
        .collect()
}

fn urging(urge: Urge, section: SectionId, state: OutlineViewState) -> impl IntoView {
    let (named, icon) = match urge {
        Urge::Earlier => ("outline-move-earlier", Icon::Earlier),
        Urge::Later => ("outline-move-later", Icon::Later),
        Urge::Promote => ("outline-promote", Icon::Promote),
        Urge::Demote => ("outline-demote", Icon::Demote),
    };
    let asked = section.clone();
    let labelled = section.clone();

    html::button()
        .r#type("button")
        .attr("aria-label", move || {
            about_section(named, &state.title_of(&labelled))
        })
        .prop("disabled", move || !state.can(&asked, urge))
        .on(ev::click, move |event| {
            event.stop_propagation();
            state.urge(section.clone(), urge);
        })
        .child(icons::mark(icon))
}

fn writing(section: SectionId, state: OutlineViewState) -> impl IntoView {
    let labelled = section.clone();
    let titled = section.clone();

    html::button()
        .r#type("button")
        .attr("aria-label", move || {
            about_section("outline-write-in", &state.title_of(&labelled))
        })
        .attr("title", move || {
            about_section("outline-write-in", &state.title_of(&titled))
        })
        .on(ev::click, move |event| {
            event.stop_propagation();
            state.write_in(section.clone());
        })
        .child(icons::mark(Icon::Quill))
}

fn pruning(section: SectionId, state: OutlineViewState) -> impl IntoView {
    let labelled = section.clone();

    html::button()
        .r#type("button")
        .attr("aria-label", move || {
            about_section("outline-remove", &state.title_of(&labelled))
        })
        .on(ev::click, move |event| {
            event.stop_propagation();
            state.remove(section.clone());
        })
        .child(icons::mark(Icon::Remove))
}

fn deed(what: String, icon: Icon, done: impl Fn() + 'static) -> impl IntoView {
    html::button()
        .r#type("button")
        .attr("aria-label", what)
        .on(ev::click, move |event| {
            event.stop_propagation();
            done();
        })
        .child(icons::mark(icon))
}

fn about_section(message: &str, title: &str) -> String {
    speech::say_about(message, "section", &shown_or_blank(title))
}

fn named_as(kind: &'static str) -> impl Fn() -> String + Send + Sync + 'static {
    move || match kind {
        "scenes" => speech::say("outline-scenes"),
        _ => speech::say("outline-ideas"),
    }
}

fn shown_or_blank(shown: &str) -> String {
    match shown.is_empty() {
        true => speech::say("untitled"),
        false => shown.to_owned(),
    }
}

fn settle(
    event: &ev::Event,
    section: &SectionId,
    held: Held,
    began_with: StoredValue<Option<String>>,
    field: NodeRef<html::Input>,
) {
    let written = event_target_value(event);
    let now = held.state.title_of(section);
    let began = began_with.get_value();

    if began.as_deref() == Some(written.as_str()) {
        if now != written {
            event_target::<HtmlInputElement>(event).set_value(&now);
        }
        return;
    }
    if now == written {
        return;
    }
    if began.is_some_and(|began| began != now) {
        let asked = RenamedMeanwhile {
            section: section.clone(),
            mine: written,
            theirs: now,
            field,
        };
        _ = set_timeout(
            move || held.renamed_meanwhile.set(Some(asked)),
            Duration::ZERO,
        );
        return;
    }
    held.state.retitle(section.clone(), written);
}

fn renamed_meanwhile(asked: RenamedMeanwhile, held: Held) -> impl IntoView {
    let close = move || held.renamed_meanwhile.set(None);
    let speech = expect_context::<Speech>();
    let told = {
        let (theirs, mine) = (asked.theirs.clone(), asked.mine.clone());

        move || {
            let mut args = FluentArgs::new();
            args.set("theirs", theirs.clone());
            args.set("mine", mine.clone());

            speech.say_with("renamed-meanwhile-told", &args)
        }
    };
    let theirs = asked.clone();
    let mine = asked;

    dialog::modal(
        Signal::derive(speech::said("renamed-meanwhile")),
        "",
        close,
        (
            html::p().child(told),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| {
                        if let Some(shown) = theirs.field.get_untracked() {
                            shown.set_value(&theirs.theirs);
                        }
                        close();
                    })
                    .child(speech::said("renamed-meanwhile-theirs")),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .on(ev::click, move |_| {
                        held.state.retitle(mine.section.clone(), mine.mine.clone());
                        close();
                    })
                    .child(speech::said("renamed-meanwhile-mine")),
            )),
        ),
    )
}
