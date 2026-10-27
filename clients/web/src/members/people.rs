use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};
use leptos_router::hooks;

use crate::accounts::{CurrentSession, SignInState};
use crate::http::ApiError;
use crate::members::listed::{Member, Role};
use crate::members::service;
use crate::problem;
use crate::projects::model::ProjectId;
use crate::route;
use crate::speech;

#[derive(Clone, Copy)]
struct People {
    project: StoredValue<String>,
    members: RwSignal<Vec<Member>>,
    refused: RwSignal<Option<ApiError>>,
}

impl People {
    fn asked(project: ProjectId) -> Self {
        let people = Self {
            project: StoredValue::new(project.to_string()),
            members: RwSignal::new(Vec::new()),
            refused: RwSignal::new(None),
        };
        people.load();

        people
    }

    fn load(self) {
        let project = self.project.get_value();
        task::spawn_local(async move {
            match service::members_of(&project).await {
                Ok(found) => self.members.set(found),
                Err(failed) => self.refused.set(Some(failed)),
            }
        });
    }

    fn change_role(self, account: String, to: Role) {
        let project = self.project.get_value();
        self.refused.set(None);
        task::spawn_local(async move {
            match service::change_role(&project, &account, to).await {
                Ok(changed) => self.members.set(changed),
                Err(failed) => {
                    self.refused.set(Some(failed));
                    self.load();
                }
            }
        });
    }

    fn remove(self, account: String, then: impl Fn() + 'static) {
        let project = self.project.get_value();
        self.refused.set(None);
        task::spawn_local(async move {
            match service::remove(&project, &account).await {
                Ok(()) => then(),
                Err(failed) => self.refused.set(Some(failed)),
            }
        });
    }
}

pub fn people(project: ProjectId) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let people = People::asked(project);
    let navigate = hooks::use_navigate();
    let left = move || navigate(route::WORKSPACE, Default::default());
    let own = move || match session.state() {
        SignInState::SignedIn(account) => Some(account.account),
        _ => None,
    };

    (
        move || people.refused.get().map(problem::problem),
        html::ul().class("members").child(move || {
            let own = own();
            let members = people.members.get();
            let owning = members.iter().any(|member| {
                member.role == Role::Owner && own.as_deref() == Some(member.account.as_str())
            });
            members
                .into_iter()
                .map(|member| {
                    let yourself = own.as_deref() == Some(member.account.as_str());
                    listed(people, member, yourself, owning, left.clone())
                })
                .collect_view()
        }),
    )
}

fn shown_name(named: &Option<String>) -> String {
    named
        .clone()
        .unwrap_or_else(|| speech::say("members-unnamed"))
}

fn listed(
    people: People,
    member: Member,
    yourself: bool,
    owning: bool,
    left: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let named = member.name.clone();
    let account = member.account.clone();
    let role = if owning {
        role_choice(people, &member).into_any()
    } else {
        html::p()
            .class("role")
            .child(speech::said(member.role.message()))
            .into_any()
    };
    let action = if yourself {
        Some(
            html::button()
                .r#type("button")
                .on(ev::click, move |_| {
                    people.remove(account.clone(), left.clone())
                })
                .child(speech::said("members-leave"))
                .into_any(),
        )
    } else {
        owning.then(|| {
            html::button()
                .r#type("button")
                .class("danger")
                .on(ev::click, move |_| {
                    people.remove(account.clone(), move || people.load())
                })
                .child(speech::said("members-remove"))
                .into_any()
        })
    };

    html::li().child((
        html::div().class("who").child((
            html::p().class("name").child((
                move || shown_name(&named),
                yourself.then(|| html::span().class("you").child(speech::said("members-you"))),
            )),
            member
                .email
                .map(|email| html::p().class("email").child(email)),
        )),
        role,
        html::div().class("actions").child(action),
    ))
}

fn role_choice(people: People, member: &Member) -> impl IntoView {
    let account = member.account.clone();
    let named = member.name.clone();
    let held = member.role;

    html::select()
        .class("role")
        .attr("aria-label", move || {
            let mut told = FluentArgs::new();
            told.set("name", shown_name(&named));
            speech::say_with("members-role-of", &told)
        })
        .on(ev::change, move |event| {
            if let Some(to) = Role::of(&event_target_value(&event)) {
                people.change_role(account.clone(), to);
            }
        })
        .child(
            Role::ALL
                .into_iter()
                .map(|offered| {
                    html::option()
                        .value(offered.named())
                        .prop("selected", offered == held)
                        .child(speech::said(offered.message()))
                })
                .collect::<Vec<_>>(),
        )
}
