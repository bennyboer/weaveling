use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};
use leptos_router::hooks;

use crate::accounts::{CurrentSession, SignInState};
use crate::http::ApiError;
use crate::members::leaving;
use crate::members::listed::{Member, Role};
use crate::members::service;
use crate::problem;
use crate::projects::model::ProjectId;
use crate::projects::service as projects;
use crate::route;
use crate::speech;

#[derive(Clone, Copy)]
pub struct People {
    session: CurrentSession,
    project: StoredValue<String>,
    pub(super) members: RwSignal<Vec<Member>>,
    refused: RwSignal<Option<ApiError>>,
    pub(super) leaving: RwSignal<bool>,
}

impl People {
    pub fn asked(project: ProjectId) -> Self {
        let people = Self {
            session: expect_context::<CurrentSession>(),
            project: StoredValue::new(project.to_string()),
            members: RwSignal::new(Vec::new()),
            refused: RwSignal::new(None),
            leaving: RwSignal::new(false),
        };
        people.load();

        people
    }

    pub(super) fn own(self) -> Option<String> {
        match self.session.state() {
            SignInState::SignedIn(account) => Some(account.account),
            _ => None,
        }
    }

    pub fn owning(self) -> bool {
        let own = self.own();
        self.members.with(|members| {
            members.iter().any(|member| {
                member.role == Role::Owner && own.as_deref() == Some(member.account.as_str())
            })
        })
    }

    fn the_only_owner(self) -> bool {
        self.owning()
            && self.members.with(|members| {
                members
                    .iter()
                    .filter(|member| member.role == Role::Owner)
                    .count()
                    == 1
            })
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

    pub(super) fn hand_over_and_leave(self, to: String, then: impl Fn() + 'static) {
        let Some(own) = self.own() else {
            return;
        };
        let project = self.project.get_value();
        self.refused.set(None);
        task::spawn_local(async move {
            let left = match service::change_role(&project, &to, Role::Owner).await {
                Ok(_) => service::remove(&project, &own).await,
                Err(failed) => Err(failed),
            };
            match left {
                Ok(()) => then(),
                Err(failed) => {
                    self.refused.set(Some(failed));
                    self.load();
                }
            }
        });
    }

    pub(super) fn delete_project(self, then: impl Fn() + 'static) {
        let project = ProjectId::from(self.project.get_value());
        self.refused.set(None);
        task::spawn_local(async move {
            match projects::delete(&project).await {
                Ok(()) => then(),
                Err(failed) => self.refused.set(Some(failed)),
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

pub fn people(people: People) -> impl IntoView {
    let navigate = hooks::use_navigate();
    let left = move || navigate(route::WORKSPACE, Default::default());

    (
        move || people.refused.get().map(problem::problem),
        {
            let left = left.clone();
            move || {
                people
                    .leaving
                    .get()
                    .then(|| leaving::leaving(people, left.clone()))
            }
        },
        html::ul().class("members").child(move || {
            let own = people.own();
            let owning = people.owning();
            people
                .members
                .get()
                .into_iter()
                .map(|member| {
                    let yourself = own.as_deref() == Some(member.account.as_str());
                    listed(people, member, yourself, owning, left.clone())
                })
                .collect_view()
        }),
    )
}

pub(super) fn shown_name(named: &Option<String>) -> String {
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
    let role = if yourself && people.the_only_owner() {
        html::p()
            .class("role")
            .child(speech::said("members-only-owner"))
            .into_any()
    } else if owning {
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
                    if people.the_only_owner() {
                        people.leaving.set(true);
                    } else {
                        people.remove(account.clone(), left.clone());
                    }
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
