use leptos::prelude::*;
use leptos::task;

use crate::act::{Act, Here};
use crate::http::ApiError;
use crate::invitations::listed::{ListedInvitation, Scope};
use crate::invitations::service;
use crate::invitations::service::InvitedInto;
use crate::members::listed::Role;

#[derive(Clone, Copy)]
pub struct Invited {
    pub(super) into: StoredValue<InvitedInto>,
    pub(super) invitations: RwSignal<Vec<ListedInvitation>>,
    pub(super) next: RwSignal<Option<String>>,
    pub(super) loading: RwSignal<bool>,
    pub(super) loaded: RwSignal<bool>,
    pub(super) refused: RwSignal<Option<ApiError>>,
    pub(super) email: RwSignal<String>,
    pub(super) role: RwSignal<Role>,
    pub(super) inviting: Act<Inviting>,
    pub(super) invite_refused: RwSignal<Option<ApiError>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Inviting {
    Typed,
    Again(String),
}

impl Invited {
    pub(super) fn asked(into: InvitedInto) -> Self {
        let invited = Self::unasked(into);
        invited.load(None);

        invited
    }

    pub fn unasked(into: InvitedInto) -> Self {
        let into = StoredValue::new(into);
        let invitations = RwSignal::new(Vec::<ListedInvitation>::new());
        let email = RwSignal::new(String::new());
        let role = RwSignal::new(Role::Author);
        let invite_refused = RwSignal::new(None);
        let inviting = Act::new(move |inviting: Inviting, _: Here| {
            invite_refused.set(None);
            let asked = match &inviting {
                Inviting::Typed => Some((
                    email.get_untracked(),
                    into.with_value(|into| scope_of(into, role.get_untracked())),
                )),
                Inviting::Again(id) => invitations.with_untracked(|listed| {
                    listed
                        .iter()
                        .find(|invitation| &invitation.id == id)
                        .map(|invitation| (invitation.email.clone(), invitation.scope.clone()))
                }),
            };

            async move {
                let Some((invited, scope)) = asked else {
                    return;
                };
                match service::invite(&invited, &scope).await {
                    Ok(invitation) => {
                        invitations.update(|listed| listed.insert(0, invitation));
                        if inviting == Inviting::Typed {
                            email.set(String::new());
                        }
                    }
                    Err(failed) => invite_refused.set(Some(failed)),
                }
            }
        });

        Self {
            into,
            invitations,
            next: RwSignal::new(None),
            loading: RwSignal::new(false),
            loaded: RwSignal::new(false),
            refused: RwSignal::new(None),
            email,
            role,
            inviting,
            invite_refused,
        }
    }

    pub fn load(self, after: Option<String>) {
        if self.loading.get_untracked() {
            return;
        }
        self.loading.set(true);
        let into = self.into.get_value();
        task::spawn_local(async move {
            match service::list_invitations(&into, after.as_deref()).await {
                Ok(page) => {
                    self.invitations
                        .update(|listed| listed.extend(page.invitations));
                    self.next.set(page.next);
                    self.loaded.set(true);
                }
                Err(failed) => self.refused.set(Some(failed)),
            }
            self.loading.set(false);
        });
    }

    pub(super) fn withdraw(self, id: String) {
        self.refused.set(None);
        task::spawn_local(async move {
            match service::withdraw(&id).await {
                Ok(()) => self
                    .invitations
                    .update(|listed| listed.retain(|invitation| invitation.id != id)),
                Err(failed) => self.refused.set(Some(failed)),
            }
        });
    }
}

fn scope_of(into: &InvitedInto, role: Role) -> Scope {
    match into {
        InvitedInto::Instance => Scope::Instance,
        InvitedInto::Project(project) => Scope::Project {
            project: project.clone(),
            role,
        },
    }
}
