use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::dialog;
use crate::members::listed::Member;
use crate::members::people::{self, People};
use crate::speech;

pub(super) fn leaving(people: People, left: impl Fn() + Clone + 'static) -> impl IntoView {
    let own = people.own();
    let others: Vec<Member> = people
        .members
        .get_untracked()
        .into_iter()
        .filter(|member| own.as_deref() != Some(member.account.as_str()))
        .collect();
    let close = move || people.leaving.set(false);

    if others.is_empty() {
        alone(people, close, left).into_any()
    } else {
        handing_over(people, others, close, left).into_any()
    }
}

fn candidate(member: &Member) -> String {
    let mut told = FluentArgs::new();
    told.set("name", people::shown_name(&member.name));
    told.set("role", speech::say(member.role.message()));

    speech::say_with("members-candidate", &told)
}

fn handing_over(
    people: People,
    others: Vec<Member>,
    close: impl Fn() + Copy + Send + Sync + 'static,
    left: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let chosen = RwSignal::new(others[0].account.clone());

    dialog::modal(
        Signal::derive(speech::said("members-last-owner-heading")),
        "leaving",
        close,
        (
            html::p()
                .class("hint")
                .child(speech::said("members-last-owner-hint")),
            html::label().class("field").child((
                html::span().child(speech::said("members-new-owner")),
                html::select()
                    .on(ev::change, move |event| {
                        chosen.set(event_target_value(&event))
                    })
                    .child(
                        others
                            .iter()
                            .map(|member| {
                                html::option()
                                    .value(member.account.clone())
                                    .child(candidate(member))
                            })
                            .collect::<Vec<_>>(),
                    ),
            )),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("members-stay")),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .on(ev::click, move |_| {
                        close();
                        people.hand_over_and_leave(chosen.get_untracked(), left.clone());
                    })
                    .child(speech::said("members-hand-over")),
            )),
        ),
    )
}

fn alone(
    people: People,
    close: impl Fn() + Copy + Send + Sync + 'static,
    left: impl Fn() + Clone + 'static,
) -> impl IntoView {
    dialog::modal(
        Signal::derive(speech::said("members-alone-heading")),
        "leaving",
        close,
        (
            html::p()
                .class("hint")
                .child(speech::said("members-alone-hint")),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("members-stay")),
                html::button()
                    .r#type("button")
                    .class("danger")
                    .on(ev::click, move |_| {
                        close();
                        people.delete_project(left.clone());
                    })
                    .child(speech::said("members-delete-project")),
            )),
        ),
    )
}
