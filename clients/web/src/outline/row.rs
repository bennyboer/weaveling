use std::time::Duration;

use fluent_bundle::FluentArgs;
use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use web_sys::HtmlInputElement;

use crate::dialog;
use crate::icons;
use crate::icons::Icon;
use crate::outline::dragging;
use crate::outline::dragging::Landing;
use crate::outline::model::{Section, SectionId};
use crate::outline::outline_view_state::{OutlineViewState, Urge};
use crate::outline::tree;
use crate::outline::tree::Held;
use crate::speech;
use crate::speech::Speech;

#[derive(Clone)]
pub(super) struct RenamedMeanwhile {
    pub(super) section: SectionId,
    pub(super) mine: String,
    pub(super) theirs: String,
    pub(super) field: NodeRef<html::Input>,
}

pub(super) fn row(section: Section, held: Held) -> impl IntoView {
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
            held.may_write
                .get_untracked()
                .then(|| dragging::grip(id.clone(), held)),
            move || {
                if !tree::bears(&bearing, held) {
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
                        tree::about_section("outline-fold", &state.title_of(&named))
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

                    move || tree::about_section("outline-section", &state.title_of(&mine))
                })
                .placeholder(speech::said("untitled"))
                .prop("readOnly", move || !held.may_write.get())
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
                    "Enter" if held.may_write.get_untracked() => {
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
                    "Tab" if held.may_write.get_untracked() => {
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
                    "ArrowUp" if event.alt_key() && held.may_write.get_untracked() => {
                        event.prevent_default();
                        state.urge(shuffled.clone(), Urge::Earlier);
                    }
                    "ArrowDown" if event.alt_key() && held.may_write.get_untracked() => {
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
                (!tree::bears(&hollowed, held)).then(|| {
                    html::span()
                        .class("hollow")
                        .child((icons::mark(Icon::Hollow), "empty"))
                })
            },
            held.may_write.get_untracked().then(|| {
                html::div()
                    .class("row-actions")
                    .attr("role", "toolbar")
                    .attr("aria-label", {
                        let mine = toolbarred.clone();

                        move || tree::about_section("outline-actions", &state.title_of(&mine))
                    })
                    .child((
                        urging(Urge::Earlier, pruned.clone(), state),
                        urging(Urge::Later, pruned.clone(), state),
                        urging(Urge::Promote, pruned.clone(), state),
                        urging(Urge::Demote, pruned.clone(), state),
                        writing(pruned.clone(), state),
                        pruning(pruned.clone(), state),
                    ))
            }),
        ))
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

pub(super) fn renamed_meanwhile(asked: RenamedMeanwhile, held: Held) -> impl IntoView {
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
            tree::about_section(named, &state.title_of(&labelled))
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
    let busy = section.clone();

    html::button()
        .r#type("button")
        .attr("aria-busy", move || state.writing_in(&busy))
        .attr("aria-label", move || {
            tree::about_section("outline-write-in", &state.title_of(&labelled))
        })
        .attr("title", move || {
            tree::about_section("outline-write-in", &state.title_of(&titled))
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
            tree::about_section("outline-remove", &state.title_of(&labelled))
        })
        .on(ev::click, move |event| {
            event.stop_propagation();
            state.remove(section.clone());
        })
        .child(icons::mark(Icon::Remove))
}
