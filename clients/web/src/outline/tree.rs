use std::collections::HashSet;

use leptos::html::{self, Li};
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::icons;
use crate::icons::Icon;
use crate::members::roles::Roles;
use crate::outline::dragging;
use crate::outline::dragging::Landing;
use crate::outline::in_view::{InView, LEAVES_PLACE};
use crate::outline::model::{Attachment, Section, SectionId};
use crate::outline::outline_view_state::OutlineViewState;
use crate::outline::row;
use crate::outline::row::RenamedMeanwhile;
use crate::problem;
use crate::route;
use crate::speech;
use crate::tray;

#[derive(Clone, Copy)]
pub(super) struct Held {
    pub(super) project: StoredValue<String>,
    pub(super) carrying: RwSignal<Option<Attachment>>,
    pub(super) hauling: RwSignal<Option<SectionId>>,
    pub(super) landing: RwSignal<Option<Landing>>,
    pub(super) over: RwSignal<Option<SectionId>>,
    pub(super) editing: RwSignal<Option<SectionId>>,
    pub(super) folded: RwSignal<HashSet<SectionId>>,
    pub(super) in_view: InView,
    pub(super) state: OutlineViewState,
    pub(super) renamed_meanwhile: RwSignal<Option<RenamedMeanwhile>>,
    pub(super) may_write: Memo<bool>,
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
    let roles = expect_context::<Roles>();
    let may_write = {
        let project = project.clone();

        Memo::new(move |_| roles.may_write(&project))
    };
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
        may_write,
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
                .map(|asked| row::renamed_meanwhile(asked, held))
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
                        .prop("disabled", move || !state.ready() || !may_write.get())
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
        .child((row::row(section, held), move || {
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

pub(super) fn bears(section: &SectionId, held: Held) -> bool {
    !held.state.attachments_in(section).is_empty() || !twigs_under(section, held).is_empty()
}

pub(super) fn icon_for(attachment: &Attachment) -> Icon {
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
            held.may_write.get_untracked().then(|| {
                deed(
                    speech::say_about("outline-take-out", "name", &shown),
                    Icon::Remove,
                    move || state.detach(taken.clone()),
                )
            }),
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

                        dragging::carried(Attachment::Scene(scene.id), shown, held)
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

                        dragging::carried(Attachment::Idea(idea.id), shown, held)
                    })
                    .collect::<Vec<_>>()
            }),
        html::p()
            .class("how")
            .child(speech::said("outline-how-to-place")),
    )
}

fn twigs_under(section: &SectionId, held: Held) -> Vec<Section> {
    held.state
        .sections()
        .into_iter()
        .filter(|held| held.parent.as_ref() == Some(section))
        .collect()
}

pub(super) fn deed(what: String, icon: Icon, done: impl Fn() + 'static) -> impl IntoView {
    html::button()
        .r#type("button")
        .attr("aria-label", what)
        .on(ev::click, move |event| {
            event.stop_propagation();
            done();
        })
        .child(icons::mark(icon))
}

pub(super) fn about_section(message: &str, title: &str) -> String {
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
