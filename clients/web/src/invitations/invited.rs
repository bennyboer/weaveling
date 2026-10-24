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
}

impl Invited {
    pub(super) fn asked() -> Self {
        let invited = Self {
            invitations: RwSignal::new(Vec::new()),
            next: RwSignal::new(None),
            loading: RwSignal::new(false),
            loaded: RwSignal::new(false),
            refused: RwSignal::new(None),
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
}
