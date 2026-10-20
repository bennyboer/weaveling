use fluent_bundle::FluentArgs;
use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::dialog::modal;
use crate::icons::{Icon, mark};
use crate::ideas::model::{Idea, IdeaId};
use crate::speech::{said, say_with};
#[derive(Clone, Copy)]
pub struct Wording {
    pub title: &'static str,
    pub listed: &'static str,
    pub every_one_taken: &'static str,
    pub confirmed: &'static str,
}

pub const LINKING: Wording = Wording {
    title: "link-ideas",
    listed: "ideas-to-link",
    every_one_taken: "every-idea-linked",
    confirmed: "link-ideas-confirm",
};

pub fn link_dialog(
    wording: Wording,
    waiting: Signal<Vec<Idea>>,
    link: impl Fn(Vec<IdeaId>) + Copy + Send + Sync + 'static,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
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

        waiting
            .get()
            .into_iter()
            .filter(|idea| idea.shown_as().to_lowercase().contains(wanted.trim()))
            .collect::<Vec<_>>()
    };

    modal(
        Signal::derive(said(wording.title)),
        "",
        close,
        (
            html::input()
                .r#type("search")
                .attr("aria-label", said("find-an-idea"))
                .attr("placeholder", said("find-an-idea"))
                .node_ref(search)
                .on(ev::input, move |event| {
                    finding.set(event_target_value(&event));
                }),
            move || {
                let found = matching();

                if found.is_empty() {
                    let why = match waiting.with(Vec::is_empty) {
                        true => wording.every_one_taken,
                        false => "no-idea-matches",
                    };

                    return html::p().class("empty").child(said(why)).into_any();
                }

                html::ul()
                    .class("choices")
                    .attr("aria-label", said(wording.listed))
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
                    .on(ev::click, move |_| close())
                    .child(said("cancel")),
                html::button()
                    .r#type("button")
                    .disabled(move || chosen.with(Vec::is_empty))
                    .on(ev::click, move |_| {
                        link(chosen.get_untracked());
                        close();
                    })
                    .child(move || {
                        let mut args = FluentArgs::new();
                        args.set("count", chosen.with(Vec::len));

                        say_with(wording.confirmed, &args)
                    }),
            )),
        ),
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
