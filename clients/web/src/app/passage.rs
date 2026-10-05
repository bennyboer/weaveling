use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::http::ApiError;
use crate::icons::{Icon, mark};
use crate::ideas::model::{Idea, IdeaId};
use crate::ideas::service as idea_service;
use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::model::PassageId;
use crate::passages::service as passage_service;
use crate::route;
use crate::shell::{Inside, masthead};

#[derive(Clone, Copy)]
struct IdeaLinks {
    project: Memo<String>,
    ideas: RwSignal<Vec<Idea>>,
    linked_ideas: RwSignal<Vec<IdeaId>>,
    picking: RwSignal<bool>,
    linking_ideas: Action<(PassageId, Vec<IdeaId>), ()>,
    unlinking_idea: Action<(PassageId, IdeaId), ()>,
}

impl IdeaLinks {
    fn named(&self, idea: &IdeaId) -> String {
        self.ideas
            .with(|known| {
                known
                    .iter()
                    .find(|held| &held.id == idea)
                    .map(|held| held.shown_as().to_owned())
            })
            .unwrap_or_else(|| "Untitled".to_owned())
    }

    fn unlinked(&self) -> Vec<Idea> {
        let linked_ideas = self.linked_ideas.get();

        self.ideas
            .get()
            .into_iter()
            .filter(|idea| !linked_ideas.contains(&idea.id))
            .collect()
    }
}

#[component]
pub fn OnePassage() -> impl IntoView {
    let params = use_params_map();
    let problem = RwSignal::new(None::<ApiError>);
    let opened = RwSignal::new(None::<PassageId>);
    let title = RwSignal::new(String::new());
    let project = Memo::new(move |_| params.read().get("project").unwrap_or_default());

    let ideas = RwSignal::new(Vec::<Idea>::new());
    let linked_ideas = RwSignal::new(Vec::<IdeaId>::new());
    let picking = RwSignal::new(false);

    let opening_passage = Action::new_local(move |asked: &PassageId| {
        let asked = asked.clone();

        async move {
            match passage_service::open(&asked).await {
                Ok(found) => {
                    problem.set(None);
                    title.set(found.title);
                    linked_ideas.set(found.ideas);
                    opened.set(Some(found.id));
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    let listing_ideas = Action::new_local(move |segment: &String| {
        let project = route::project_id(segment);

        async move {
            match idea_service::list(&project).await {
                Ok(found) => ideas.set(found),
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    let retitling_passage = Action::new_local(move |(passage, typed): &(PassageId, String)| {
        let passage = passage.clone();
        let typed = typed.clone();

        async move {
            match passage_service::retitle(&passage, &typed).await {
                Ok(retitled) => {
                    problem.set(None);
                    title.set(retitled.title);
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    let linking_ideas = Action::new_local(move |(passage, chosen): &(PassageId, Vec<IdeaId>)| {
        let passage = passage.clone();
        let chosen = chosen.clone();

        async move {
            for idea in &chosen {
                match passage_service::link(&passage, idea).await {
                    Ok(updated) => linked_ideas.set(updated.ideas),
                    Err(failure) => return problem.set(Some(failure)),
                }
            }
            problem.set(None);
        }
    });

    let unlinking_idea = Action::new_local(move |(passage, idea): &(PassageId, IdeaId)| {
        let passage = passage.clone();
        let idea = idea.clone();

        async move {
            match passage_service::unlink(&passage, &idea).await {
                Ok(updated) => {
                    problem.set(None);
                    linked_ideas.set(updated.ideas);
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    let links = IdeaLinks {
        project,
        ideas,
        linked_ideas,
        picking,
        linking_ideas,
        unlinking_idea,
    };

    Effect::new(move || {
        if let Some(asked) = params.read().get("passage") {
            opening_passage.dispatch(PassageId::from(asked));
        }
    });

    Effect::new(move || {
        listing_ideas.dispatch(project.get());
    });

    window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            picking.set(false);
        }
    });

    (
        move || masthead(Some(Inside::of(&project.get(), None))),
        html::main().child(html::div().class("column").child((
            move || {
                problem.get().map(|failure| {
                    html::p()
                        .class("problem")
                        .role("alert")
                        .child(failure.to_string())
                })
            },
            move || {
                match opened.get() {
                    Some(passage) => (
                        PassageEditor(PassageEditorProps {
                            passage: passage.clone(),
                        }),
                        about(passage.clone(), title, retitling_passage),
                        drawn_from(passage.clone(), links),
                        move || picking.get().then(|| picker(passage.clone(), links)),
                    )
                        .into_any(),
                    None => html::p().class("empty").child("Opening…").into_any(),
                }
            },
        ))),
    )
}

fn about(
    passage: PassageId,
    title: RwSignal<String>,
    retitling_passage: Action<(PassageId, String), ()>,
) -> impl IntoView {
    html::section().class("about").child(
        html::input()
            .r#type("text")
            .class("passage-title")
            .attr("aria-label", "Passage title")
            .attr(
                "placeholder",
                "Untitled \u{2014} the outline shows its opening words",
            )
            .prop("value", move || title.get())
            .on(ev::change, move |event| {
                let Some(field) = typed(&event) else {
                    return;
                };

                retitling_passage.dispatch((passage.clone(), field.value()));
            }),
    )
}

fn drawn_from(passage: PassageId, links: IdeaLinks) -> impl IntoView {
    html::section().class("drawn-from").child((
        html::h2().child("Drawn from"),
        move || {
            let linked_ideas = links.linked_ideas.get();

            if linked_ideas.is_empty() {
                return html::p()
                    .class("empty")
                    .child("No ideas linked yet.")
                    .into_any();
            }

            html::ul()
                .attr("aria-label", "Drawn from")
                .child(
                    linked_ideas
                        .into_iter()
                        .map(|idea| linked_idea(passage.clone(), idea, links))
                        .collect::<Vec<_>>(),
                )
                .into_any()
        },
        html::button()
            .r#type("button")
            .class("link-an-idea")
            .on(ev::click, move |_| links.picking.set(true))
            .child((mark(Icon::Plus), "Link an idea")),
    ))
}

fn linked_idea(passage: PassageId, idea: IdeaId, links: IdeaLinks) -> impl IntoView {
    let named = links.named(&idea);
    let at = route::idea(&links.project.get_untracked(), &idea, &named);
    let shown = named.clone();

    html::li().child((
        mark(Icon::Idea),
        view! {
            <A href=at attr:class="name">
                {shown}
            </A>
        },
        html::button()
            .r#type("button")
            .attr("aria-label", format!("Unlink {named}"))
            .attr("title", format!("Unlink {named}"))
            .on(ev::click, move |_| {
                links
                    .unlinking_idea
                    .dispatch((passage.clone(), idea.clone()));
            })
            .child(mark(Icon::Remove)),
    ))
}

fn picker(passage: PassageId, links: IdeaLinks) -> impl IntoView {
    let finding = RwSignal::new(String::new());
    let chosen = RwSignal::new(Vec::<IdeaId>::new());
    let search = NodeRef::<html::Input>::new();

    Effect::new(move || {
        if let Some(field) = search.get() {
            let _ = field.focus();
        }
    });

    let matching = move || {
        let wanted = finding.get().to_lowercase();

        links
            .unlinked()
            .into_iter()
            .filter(|idea| idea.shown_as().to_lowercase().contains(wanted.trim()))
            .collect::<Vec<_>>()
    };

    html::div()
        .class("overlay")
        .on(ev::click, move |_| links.picking.set(false))
        .child(
            html::div()
                .class("dialog")
                .role("dialog")
                .attr("aria-modal", "true")
                .attr("aria-label", "Link ideas")
                .on(ev::click, |event| event.stop_propagation())
                .child((
                    html::h2().child("Link ideas"),
                    html::input()
                        .r#type("search")
                        .attr("aria-label", "Find an idea")
                        .attr("placeholder", "Find an idea")
                        .node_ref(search)
                        .on(ev::input, move |event| {
                            if let Some(field) = typed(&event) {
                                finding.set(field.value());
                            }
                        }),
                    move || {
                        let found = matching();

                        if found.is_empty() {
                            let why = match links.unlinked().is_empty() {
                                true => "Every idea in this project is already linked.",
                                false => "No idea matches.",
                            };

                            return html::p().class("empty").child(why).into_any();
                        }

                        html::ul()
                            .class("choices")
                            .attr("aria-label", "Ideas to link")
                            .child(
                                found
                                    .into_iter()
                                    .map(|idea| choice(idea, chosen))
                                    .collect::<Vec<_>>(),
                            )
                            .into_any()
                    },
                    html::div().class("dialog-actions").child((
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| links.picking.set(false))
                            .child("Cancel"),
                        html::button()
                            .r#type("button")
                            .disabled(move || chosen.with(Vec::is_empty))
                            .on(ev::click, move |_| {
                                links
                                    .linking_ideas
                                    .dispatch((passage.clone(), chosen.get_untracked()));
                                links.picking.set(false);
                            })
                            .child(move || match chosen.with(Vec::len) {
                                0 | 1 => "Link".to_owned(),
                                many => format!("Link {many}"),
                            }),
                    )),
                )),
        )
}

fn choice(idea: Idea, chosen: RwSignal<Vec<IdeaId>>) -> impl IntoView {
    let shown = idea.shown_as().to_owned();
    let mine = idea.id.clone();
    let toggled = idea.id;

    html::li().child(
        html::label().child((
            html::input()
                .r#type("checkbox")
                .prop("checked", move || chosen.with(|held| held.contains(&mine)))
                .on(ev::change, move |_| {
                    chosen.update(|held| match held.iter().position(|it| it == &toggled) {
                        Some(at) => {
                            held.remove(at);
                        }
                        None => held.push(toggled.clone()),
                    })
                }),
            mark(Icon::Idea),
            shown,
        )),
    )
}

fn typed(event: &ev::Event) -> Option<HtmlInputElement> {
    event
        .target()
        .and_then(|it| it.dyn_into::<HtmlInputElement>().ok())
}
