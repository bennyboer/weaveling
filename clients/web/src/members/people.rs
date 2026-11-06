use std::future::Future;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};
use leptos_router::hooks;

use crate::accounts::{CurrentSession, SignInState};
use crate::http::ApiError;
use crate::live::Subscription;
use crate::members::intent::Intent;
use crate::members::leaving;
use crate::members::listed::{Member, Members, Role};
use crate::members::service;
use crate::problem;
use crate::projects::intent::Intent as ProjectIntent;
use crate::projects::model::ProjectId;
use crate::projects::open_project::OpenProject;
use crate::route;
use crate::speech;
use crate::working_copy::WorkingCopy;
use live_contract::FrameDTO;

#[derive(Clone, Copy)]
pub struct People {
    session: CurrentSession,
    open_project: OpenProject,
    project: StoredValue<String>,
    working: WorkingCopy<Members, Intent>,
    refused: RwSignal<Option<ApiError>>,
    pub(super) leaving: RwSignal<bool>,
    following: StoredValue<Option<Subscription>, LocalStorage>,
}

impl People {
    pub fn asked(project: ProjectId) -> Self {
        let project = project.to_string();
        let refused = RwSignal::new(None);
        let (sending_to, reading) = (project.clone(), project.clone());
        let working = WorkingCopy::new(
            move |on: Members, intent: Intent| {
                let project = sending_to.clone();

                async move {
                    match &intent {
                        Intent::ChangeRole { account, to } => {
                            service::change_role(&project, account, *to, on.version).await
                        }
                        Intent::Remove { account } => {
                            service::remove(&project, account, on.version).await?;
                            Ok(service::members_of(&project)
                                .await
                                .ok()
                                .filter(|read| read.version > on.version)
                                .unwrap_or_else(|| intent.landed_on(&on)))
                        }
                    }
                }
            },
            move || {
                let project = reading.clone();

                async move { service::members_of(&project).await }
            },
            move |failure| _ = refused.try_set(Some(failure)),
        );
        let people = Self {
            session: expect_context::<CurrentSession>(),
            open_project: expect_context::<OpenProject>(),
            project: StoredValue::new(project),
            working,
            refused,
            leaving: RwSignal::new(false),
            following: StoredValue::new_local(None),
        };
        people.following.set_value(Some(Subscription::start(
            "members",
            &[("project", &people.project.get_value())],
            move |_: FrameDTO| people.working.read_again(),
            move || people.working.read_again(),
        )));

        people
    }

    pub(super) fn own(self) -> Option<String> {
        match self.session.state() {
            SignInState::SignedIn(account) => Some(account.account),
            _ => None,
        }
    }

    pub(super) fn members(self) -> Vec<Member> {
        self.working
            .shown()
            .map(|members| members.people)
            .unwrap_or_default()
    }

    pub fn owning(self) -> bool {
        let own = self.own();
        self.members().iter().any(|member| {
            member.role == Role::Owner && own.as_deref() == Some(member.account.as_str())
        })
    }

    fn the_only_owner(self) -> bool {
        self.owning()
            && self
                .members()
                .iter()
                .filter(|member| member.role == Role::Owner)
                .count()
                == 1
    }

    fn change_role(self, account: String, to: Role) {
        self.refused.set(None);
        self.working.intend(Intent::ChangeRole { account, to });
    }

    pub(super) fn hand_over_and_leave(self, to: String, then: impl Fn() + 'static) {
        let Some(own) = self.own() else {
            return;
        };
        self.refused.set(None);
        let working = self.working;
        let handed = working.intended(Intent::ChangeRole {
            account: to,
            to: Role::Owner,
        });
        self.once_landed(
            async move {
                handed.await?;
                working.intended(Intent::Remove { account: own }).await
            },
            then,
        );
    }

    pub(super) fn trash_project(self, then: impl Fn() + 'static) {
        let project = ProjectId::from(self.project.get_value());
        self.refused.set(None);
        self.once_landed(
            self.open_project.intended(&project, ProjectIntent::Trash),
            then,
        );
    }

    fn leave(self, then: impl Fn() + 'static) {
        let Some(own) = self.own() else {
            return;
        };
        self.refused.set(None);
        self.once_landed(self.working.intended(Intent::Remove { account: own }), then);
    }

    fn once_landed(
        self,
        landing: impl Future<Output = Result<(), ApiError>> + 'static,
        then: impl FnOnce() + 'static,
    ) {
        task::spawn_local(async move {
            match landing.await {
                Ok(()) => then(),
                Err(failure) => _ = self.refused.try_set(Some(failure)),
            }
        });
    }

    fn remove(self, account: String) {
        self.refused.set(None);
        self.working.intend(Intent::Remove { account });
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
                .members()
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
                        people.leave(left.clone());
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
                .on(ev::click, move |_| people.remove(account.clone()))
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
