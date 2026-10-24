use leptos::prelude::*;
use leptos::task;

use crate::http::ApiError;
use crate::invitations::listed::ListedInvitation;
use crate::invitations::service;

#[derive(Clone, Copy)]
pub(super) struct Invited {
    pub(super) invitations: RwSignal<Vec<ListedInvitation>>,
    pub(super) next: RwSignal<Option<String>>,
    pub(super) loading: RwSignal<bool>,
    pub(super) loaded: RwSignal<bool>,
    pub(super) refused: RwSignal<Option<ApiError>>,
    pub(super) inviting: RwSignal<bool>,
    pub(super) invite_refused: RwSignal<Option<ApiError>>,
}

impl Invited {
    pub(super) fn asked() -> Self {
        let invited = Self {
            invitations: RwSignal::new(Vec::new()),
            next: RwSignal::new(None),
            loading: RwSignal::new(false),
            loaded: RwSignal::new(false),
            refused: RwSignal::new(None),
            inviting: RwSignal::new(false),
            invite_refused: RwSignal::new(None),
        };
        invited.load(None);

        invited
    }

    pub(super) fn load(self, after: Option<String>) {
        if self.loading.get_untracked() {
            return;
        }
        self.loading.set(true);
        task::spawn_local(async move {
            match service::list_invitations(after.as_deref()).await {
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

    pub(super) fn invite(self, email: String, done: impl Fn() + 'static) {
        if self.inviting.get_untracked() {
            return;
        }
        self.inviting.set(true);
        self.invite_refused.set(None);
        task::spawn_local(async move {
            match service::invite(&email).await {
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
