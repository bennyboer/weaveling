use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::invitations::invited::Invited;
use crate::invitations::listed::{DeclineReason, ListedInvitation, Status};
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

pub fn invitations_listing() -> impl IntoView {
    let invited = Invited::asked();

    (
        inviting(invited),
        move || invited.invite_refused.get().map(problem::problem),
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

    html::form()
        .class("invite")
        .on(ev::submit, move |event| {
            event.prevent_default();
            invited.invite(email.get_untracked(), move || email.set(String::new()));
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
    let again = (invitation.status == Status::Expired).then(|| invitation.email.clone());
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
    };

    html::li().child((
        html::div().class("who").child((
            html::p().class("email").child(invitation.email),
            html::p().class("issued").child(issued),
        )),
        html::div()
            .class("marks")
            .child(html::span().class(mark).child(told)),
        html::div().class("actions").child((
            again.map(|email| {
                html::button()
                    .r#type("button")
                    .disabled(move || invited.inviting.get())
                    .on(ev::click, move |_| invited.invite(email.clone(), || {}))
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
