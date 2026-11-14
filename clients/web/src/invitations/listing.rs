use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::avatar;
use crate::blocks;
use crate::invitations::invited::{Invited, Inviting};
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

/// The invitations as a list card, with the form to invite someone at its top.
/// An instance's invitations are headed "Invitations", a project's "Invited".
pub fn invitations_listing(into: InvitedInto) -> impl IntoView {
    let heading = match into {
        InvitedInto::Instance => "instance-invitations-heading",
        InvitedInto::Project(_) => "members-invited",
    };
    let invited = Invited::asked(into);

    blocks::list_card(
        "invitations",
        speech::said(heading),
        (),
        (
            html::div().class("inviting").child(invite_form(invited)),
            invited_list(invited),
        ),
        (),
    )
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
            (invited.invitations.is_read() && invited.invitations.shown().is_empty()).then(|| {
                html::p()
                    .class("hint")
                    .child(speech::said("instance-invitations-none"))
            })
        },
        html::div()
            .class("invitations")
            .child(blocks::rows(move || {
                invited
                    .invitations
                    .shown()
                    .into_iter()
                    .map(|invitation| listed(invited, invitation))
                    .collect_view()
            })),
        move || {
            invited.next.get().map(|next| {
                blocks::more(
                    "instance-invitations-more",
                    move || invited.loading.get(),
                    move |_| invited.load(Some(next.clone())),
                )
            })
        },
    )
}

fn inviting(invited: Invited) -> impl IntoView {
    let (email, role) = (invited.email, invited.role);
    let into_a_project = matches!(invited.into.get_value(), InvitedInto::Project(_));

    html::form()
        .class("invite")
        .on(ev::submit, move |event| {
            event.prevent_default();
            invited.inviting.start(Inviting::Typed);
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
            into_a_project.then(|| {
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
                .attr("aria-busy", move || {
                    invited.inviting.busy_on(&Inviting::Typed)
                })
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
    let again = (invitation.status == Status::Expired).then(|| Inviting::Again(id.clone()));
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

    blocks::row(
        avatar::avatar(&invitation.email, Some(&invitation.email), None),
        invitation.email.clone(),
        issued,
        (
            html::span().class(mark).child(told),
            match &invitation.scope {
                Scope::Instance => None,
                Scope::Project { role, .. } => Some(
                    html::span()
                        .class("mark")
                        .child(speech::said(role.message())),
                ),
            },
        ),
        (
            again.map(|again| {
                let pressed = again.clone();

                html::button()
                    .r#type("button")
                    .attr("aria-busy", move || invited.inviting.busy_on(&pressed))
                    .on(ev::click, move |_| invited.inviting.start(again.clone()))
                    .child(speech::said("instance-invitation-invite-again"))
            }),
            withdrawable.then(|| {
                let busy = id.clone();

                html::button()
                    .r#type("button")
                    .attr("aria-busy", move || {
                        invited.invitations.unanswered_on(&busy).then_some("true")
                    })
                    .on(ev::click, move |_| invited.withdraw(id.clone()))
                    .child(speech::said("instance-invitation-withdraw"))
            }),
        ),
    )
}
