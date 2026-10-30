use std::collections::BTreeMap;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use live_contract::FrameDTO;
use members_contract::MembersEventDTO;

use crate::accounts::{CurrentSession, SignInState};
use crate::live::Subscription;
use crate::members::listed::Role;
use crate::members::service;
use crate::quiet_note::QuietNote;
use crate::route;
use crate::speech::Speech;

#[derive(Clone, Copy)]
pub struct Roles {
    signed_in: Memo<bool>,
    held: RwSignal<Option<BTreeMap<String, Role>>>,
    following: StoredValue<Option<Subscription>, LocalStorage>,
}

impl Roles {
    pub fn followed() -> Self {
        let session = expect_context::<CurrentSession>();
        let roles = Self {
            signed_in: Memo::new(move |_| matches!(session.state(), SignInState::SignedIn(_))),
            held: RwSignal::new(None),
            following: StoredValue::new_local(None),
        };
        let note = expect_context::<QuietNote>();
        let speech = expect_context::<Speech>();

        Effect::new(move |_| {
            roles.held.set(None);
            roles.following.set_value(roles.signed_in.get().then(|| {
                Subscription::start(
                    "members",
                    &[("mine", "true")],
                    move |frame| {
                        roles.heard(&frame, note, speech);
                        roles.ask();
                    },
                    move || roles.ask(),
                )
            }));
        });

        roles
    }

    fn ask(self) {
        task::spawn_local(async move {
            if let Ok(held) = service::mine().await {
                _ = self.held.try_set(Some(held.into_iter().collect()));
            }
        });
    }

    fn heard(self, frame: &FrameDTO, note: QuietNote, speech: Speech) {
        if let Ok(MembersEventDTO::RoleChanged { to, .. }) =
            serde_json::from_value::<MembersEventDTO>(frame.event.clone())
            && let Some(role) = Role::of(&to)
        {
            let mut told = FluentArgs::new();
            told.set("role", speech.say(role.message()));
            note.tell(speech.say_with("roles-changed", &told));
        }
    }

    fn role_in(self, project: &str) -> Option<Option<Role>> {
        if !self.signed_in.get() {
            return None;
        }

        let project = route::project_id(project).to_string();

        self.held
            .with(|held| held.as_ref().map(|held| held.get(&project).copied()))
    }

    pub fn may_write(self, project: &str) -> bool {
        match self.role_in(project) {
            None => true,
            Some(role) => matches!(role, Some(Role::Owner | Role::Author)),
        }
    }

    pub fn owns(self, project: &str) -> bool {
        match self.role_in(project) {
            None => true,
            Some(role) => role == Some(Role::Owner),
        }
    }
}
