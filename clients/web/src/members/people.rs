use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, html};

use crate::accounts::{CurrentSession, SignInState};
use crate::http::ApiError;
use crate::members::listed::Member;
use crate::members::service;
use crate::problem;
use crate::projects::model::ProjectId;
use crate::speech;

pub fn people(project: ProjectId) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let members = RwSignal::new(Vec::<Member>::new());
    let refused = RwSignal::new(None::<ApiError>);
    task::spawn_local(async move {
        match service::members_of(&project.to_string()).await {
            Ok(found) => members.set(found),
            Err(failed) => refused.set(Some(failed)),
        }
    });
    let own = move || match session.state() {
        SignInState::SignedIn(account) => Some(account.account),
        _ => None,
    };

    (
        move || refused.get().map(problem::problem),
        html::ul().class("members").child(move || {
            let own = own();
            members
                .get()
                .into_iter()
                .map(|member| {
                    let yourself = own.as_deref() == Some(member.account.as_str());
                    listed(member, yourself)
                })
                .collect_view()
        }),
    )
}

fn listed(member: Member, yourself: bool) -> impl IntoView {
    let named = member.name.clone();

    html::li().child((
        html::div().class("who").child((
            html::p().class("name").child((
                move || {
                    named
                        .clone()
                        .unwrap_or_else(|| speech::say("members-unnamed"))
                },
                yourself.then(|| html::span().class("you").child(speech::said("members-you"))),
            )),
            member
                .email
                .map(|email| html::p().class("email").child(email)),
        )),
        html::p()
            .class("role")
            .child(speech::said(member.role.message())),
    ))
}
