use std::collections::{BTreeMap, HashMap};

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, html};
use serde_json::json;

use crate::accounts::{CurrentSession, SignInState};
use crate::avatar;
use crate::members::listed::Member;
use crate::members::service as members;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::speech;

/// Who else is writing in which scene of the book.
/// Each scene's editor tells which accounts have their cursor in it.
/// The account viewing is left out, and an account in several tabs counts once.
/// The members of the project give each account its name, picture and tint.
#[derive(Clone, Copy)]
pub struct Presence {
    project: StoredValue<String>,
    own: StoredValue<String>,
    people: RwSignal<HashMap<String, Member>>,
    here: RwSignal<HashMap<SceneId, Vec<String>>>,
    asking: StoredValue<bool>,
}

impl Presence {
    /// Knows nobody yet, and asks for the project's members if an account is signed in.
    /// Without accounts there is nobody else, so nothing is asked.
    pub fn of(project: &ProjectId) -> Self {
        let own = match untrack(|| expect_context::<CurrentSession>().state()) {
            SignInState::SignedIn(signed_in) => signed_in.account,
            _ => String::new(),
        };
        let presence = Self {
            project: StoredValue::new(project.to_string()),
            own: StoredValue::new(own),
            people: RwSignal::new(HashMap::new()),
            here: RwSignal::new(HashMap::new()),
            asking: StoredValue::new(false),
        };
        if !presence.own().is_empty() {
            presence.ask_for_people();
        }

        presence
    }

    /// The account viewing, or nothing without accounts.
    pub fn own(self) -> String {
        self.own.get_value()
    }

    /// Takes in who has their cursor in a scene now, as its editor tells it.
    /// An account not known among the members makes them be asked again, as it may have joined just now.
    pub fn told(self, scene: &SceneId, accounts: Vec<String>) {
        let own = self.own();
        let mut others: Vec<String> = accounts
            .into_iter()
            .filter(|account| *account != own)
            .collect();
        others.sort();
        others.dedup();
        let unknown = self
            .people
            .with_untracked(|people| others.iter().any(|account| !people.contains_key(account)));
        self.here.update(|here| {
            if others.is_empty() {
                here.remove(scene);
            } else {
                here.insert(scene.clone(), others);
            }
        });
        if unknown {
            self.ask_for_people();
        }
    }

    /// Forgets who was in a scene whose editor is gone.
    pub fn let_go_of(self, scene: &SceneId) {
        self.here.update(|here| {
            here.remove(scene);
        });
    }

    /// Who else is in the scene, by account, with what the members tell of them, ordered by name.
    pub fn in_scene(self, scene: &SceneId) -> Vec<(String, Option<Member>)> {
        let accounts = self
            .here
            .with(|here| here.get(scene).cloned().unwrap_or_default());
        let mut shown: Vec<(String, Option<Member>)> = self.people.with(|people| {
            accounts
                .into_iter()
                .map(|account| {
                    let member = people.get(&account).cloned();
                    (account, member)
                })
                .collect()
        });
        shown.sort_by_key(|(account, member)| {
            (
                member.as_ref().and_then(|member| member.name.clone()),
                account.clone(),
            )
        });

        shown
    }

    /// The members as the editors need them for the names and colours of others' cursors, as JSON.
    /// Each account is told with its name and the tint its initial is drawn in.
    pub fn people_told(self) -> String {
        self.people.with(|people| {
            let told: BTreeMap<&String, serde_json::Value> = people
                .iter()
                .map(|(account, member)| {
                    (
                        account,
                        json!({
                            "name": member.name.clone().unwrap_or_default(),
                            "tint": avatar::tint_of(account),
                        }),
                    )
                })
                .collect();

            json!(told).to_string()
        })
    }

    fn ask_for_people(self) {
        if self.asking.get_value() {
            return;
        }
        self.asking.set_value(true);
        let project = self.project.get_value();
        task::spawn_local(async move {
            if let Ok(found) = members::members_of(&project).await {
                _ = self.people.try_set(
                    found
                        .people
                        .into_iter()
                        .map(|member| (member.account.clone(), member))
                        .collect(),
                );
            }
            _ = self.asking.try_set_value(false);
        });
    }
}

/// The pictures of whoever else is writing in a scene, side by side in its margin.
/// Each tells its name on hovering it, and to screen readers.
pub fn shown(scene: SceneId, presence: Presence) -> impl IntoView {
    move || {
        let here = presence.in_scene(&scene);

        (!here.is_empty()).then(|| {
            html::div()
                .class("scene-presence")
                .role("list")
                .attr("aria-label", speech::say("presence-here"))
                .child(
                    here.into_iter()
                        .map(|(account, member)| {
                            let name = member
                                .as_ref()
                                .and_then(|member| member.name.clone())
                                .unwrap_or_else(|| speech::say("members-unnamed"));
                            let mut told = FluentArgs::new();
                            told.set("name", name.clone());
                            let writing = speech::say_with("presence-writing", &told);

                            html::span()
                                .class("present")
                                .role("listitem")
                                .attr("title", writing.clone())
                                .attr("aria-label", writing)
                                .child(avatar::avatar(
                                    &account,
                                    Some(&name),
                                    member.as_ref().and_then(|member| member.picture.as_deref()),
                                ))
                        })
                        .collect_view(),
                )
        })
    }
}
