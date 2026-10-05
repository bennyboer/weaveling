use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::icons::{Icon, mark};
use crate::ideas::model::{Idea, IdeaId};
use crate::inputs::typed;
use crate::passages::open_passage::OpenPassage;

pub fn link_dialog(open: OpenPassage) -> impl IntoView {
    let finding = RwSignal::new(String::new());
    let chosen = RwSignal::new(Vec::<IdeaId>::new());
    let search = NodeRef::<html::Input>::new();

    Effect::new(move || {
        if let Some(field) = search.get() {
            let _ = field.focus();
        }
    });

    window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            open.stop_picking();
        }
    });

    let matching = move || {
        let wanted = finding.get().to_lowercase();

        open.waiting_ideas()
            .into_iter()
            .filter(|idea| idea.shown_as().to_lowercase().contains(wanted.trim()))
            .collect::<Vec<_>>()
    };

    html::div()
        .class("overlay")
        .on(ev::click, move |_| open.stop_picking())
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
                            let why = match open.waiting_ideas().is_empty() {
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
                            .on(ev::click, move |_| open.stop_picking())
                            .child("Cancel"),
                        html::button()
                            .r#type("button")
                            .disabled(move || chosen.with(Vec::is_empty))
                            .on(ev::click, move |_| {
                                open.link(chosen.get_untracked());
                                open.stop_picking();
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
