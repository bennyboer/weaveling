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
        html::h2().child(speech::said("instance-invitations-heading")),
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
                .map(listed)
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

fn listed(invitation: ListedInvitation) -> impl IntoView {
    let issued = said_with(
        "instance-invitation-issued",
        &[("when", moment::human_time(invitation.issued_at))],
    );
    let until = moment::human_time(invitation.until);
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
    ))
}
