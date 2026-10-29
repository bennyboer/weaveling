use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::invitations::invited::Invited;
use crate::invitations::listed::{DeclineReason, ListedInvitation, Scope, Status};
use crate::invitations::service::InvitedInto;
use crate::members::listed::Role;
use crate::moment;
use crate::problem;
use crate::speech;

fn said_with(key: &'static str, args: &[(&'static str, String)]) -> String {
    let mut told = FluentArgs::new();
    for (name, value) in args {
        told.set(*name, value.clone());
    }

    speech::say_with(key, &told)
}

pub fn invitations_listing(into: InvitedInto) -> impl IntoView {
    let invited = Invited::asked(into);

    (invite_form(invited), invited_list(invited))
}

pub fn invite_form(invited: Invited) -> impl IntoView {
    (inviting(invited), move || {
        invited.invite_refused.get().map(problem::problem)
    })
}

pub fn invited_list(invited: Invited) -> impl IntoView {
    (
        move || invited.refused.get().map(problem::problem),
        move || {
            (invited.loaded.get() && invited.invitations.with(Vec::is_empty)).then(|| {
                html::p()
                    .class("hint")
                    .child(speech::said("instance-invitations-none"))
            })
        },
        html::ul().class("invitations").child(move || {
            invited
                .invitations
                .get()
                .into_iter()
                .map(|invitation| listed(invited, invitation))
                .collect_view()
        }),
        move || {
            invited.next.get().map(|next| {
                html::button()
                    .r#type("button")
                    .class("more")
                    .disabled(move || invited.loading.get())
                    .on(ev::click, move |_| invited.load(Some(next.clone())))
                    .child(speech::said("instance-invitations-more"))
            })
        },
    )
}

fn inviting(invited: Invited) -> impl IntoView {
    let email = RwSignal::new(String::new());
    let role = RwSignal::new(Role::Author);
    let into = invited.into.get_value();
    let into_a_project = match &into {
        InvitedInto::Instance => None,
        InvitedInto::Project(project) => Some(project.clone()),
    };
    let scope = {
        let into_a_project = into_a_project.clone();
        move || match &into_a_project {
            None => Scope::Instance,
            Some(project) => Scope::Project {
                project: project.clone(),
                role: role.get_untracked(),
            },
        }
    };

    html::form()
        .class("invite")
        .on(ev::submit, move |event| {
            event.prevent_default();
            invited.invite(email.get_untracked(), scope(), move || {
                email.set(String::new())
            });
        })
        .child((
            html::label().class("field").child((
                html::span().child(speech::said("instance-invite-email")),
                html::input()
                    .r#type("email")
                    .attr("autocomplete", "off")
                    .required(true)
                    .prop("value", move || email.get())
                    .on(ev::input, move |event| {
                        email.set(event_target_value(&event))
                    }),
            )),
            into_a_project.map(|_| {
                html::label().class("field as").child((
                    html::span().child(speech::said("members-invite-as")),
                    html::select()
                        .on(ev::change, move |event| {
                            if let Some(chosen) = Role::of(&event_target_value(&event)) {
                                role.set(chosen);
                            }
                        })
                        .child(
                            Role::ALL
                                .into_iter()
                                .map(|offered| {
                                    html::option()
                                        .value(offered.named())
                                        .prop("selected", move || role.get() == offered)
                                        .child(speech::said(offered.message()))
                                })
                                .collect::<Vec<_>>(),
                        ),
                ))
            }),
            html::button()
                .r#type("submit")
                .class("primary")
                .disabled(move || invited.inviting.get())
                .child(speech::said("instance-invite-confirm")),
        ))
}

fn listed(invited: Invited, invitation: ListedInvitation) -> impl IntoView {
    let issued = said_with(
        "instance-invitation-issued",
        &[("when", moment::human_time(invitation.issued_at))],
    );
    let until = moment::human_time(invitation.until);
    let withdrawable = invitation.status == Status::Open;
    let id = invitation.id.clone();
    let again = (invitation.status == Status::Expired)
        .then(|| (invitation.email.clone(), invitation.scope.clone()));
    let (mark, told) = match invitation.status {
        Status::Open => (
            "mark",
            said_with("instance-invitation-open", &[("when", until)]),
        ),
        Status::Expired => (
            "mark expired",
            said_with("instance-invitation-expired", &[("when", until)]),
        ),
        Status::Declined(DeclineReason::AddressHasAccount) => (
            "mark declined",
            speech::say("instance-invitation-declined-address-has-account"),
        ),
        Status::Declined(DeclineReason::TurnedDown) => (
            "mark declined",
            speech::say("instance-invitation-declined-turned-down"),
        ),
    };

    html::li().child((
        html::div().class("who").child((
            html::p().class("email").child(invitation.email),
            html::p().class("issued").child(issued),
        )),
        html::div().class("marks").child((
            html::span().class(mark).child(told),
            match &invitation.scope {
                Scope::Instance => None,
                Scope::Project { role, .. } => Some(
                    html::span()
                        .class("mark")
                        .child(speech::said(role.message())),
                ),
            },
        )),
        html::div().class("actions").child((
            again.map(|(email, scope)| {
                html::button()
                    .r#type("button")
                    .disabled(move || invited.inviting.get())
                    .on(ev::click, move |_| {
                        invited.invite(email.clone(), scope.clone(), || {})
                    })
                    .child(speech::said("instance-invitation-invite-again"))
            }),
            withdrawable.then(|| {
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| invited.withdraw(id.clone()))
                    .child(speech::said("instance-invitation-withdraw"))
            }),
        )),
    ))
}
