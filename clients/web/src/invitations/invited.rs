use leptos::prelude::*;
use leptos::task;

use crate::http::ApiError;
use crate::invitations::listed::{ListedInvitation, Scope};
use crate::invitations::service;
use crate::invitations::service::InvitedInto;

#[derive(Clone, Copy)]
pub struct Invited {
    pub(super) into: StoredValue<InvitedInto>,
    pub(super) invitations: RwSignal<Vec<ListedInvitation>>,
    pub(super) next: RwSignal<Option<String>>,
    pub(super) loading: RwSignal<bool>,
    pub(super) loaded: RwSignal<bool>,
    pub(super) refused: RwSignal<Option<ApiError>>,
    pub(super) inviting: RwSignal<bool>,
    pub(super) invite_refused: RwSignal<Option<ApiError>>,
}

impl Invited {
    pub(super) fn asked(into: InvitedInto) -> Self {
        let invited = Self::unasked(into);
        invited.load(None);

        invited
    }

    pub fn unasked(into: InvitedInto) -> Self {
        Self {
            into: StoredValue::new(into),
            invitations: RwSignal::new(Vec::new()),
            next: RwSignal::new(None),
            loading: RwSignal::new(false),
            loaded: RwSignal::new(false),
            refused: RwSignal::new(None),
            inviting: RwSignal::new(false),
            invite_refused: RwSignal::new(None),
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

    pub(super) fn invite(self, email: String, scope: Scope, done: impl Fn() + 'static) {
        if self.inviting.get_untracked() {
            return;
        }
        self.inviting.set(true);
        self.invite_refused.set(None);
        task::spawn_local(async move {
            match service::invite(&email, &scope).await {
                Ok(invitation) => {
                    self.invitations
                        .update(|listed| listed.insert(0, invitation));
                    done();
                }
                Err(failed) => self.invite_refused.set(Some(failed)),
            }
            self.inviting.set(false);
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
