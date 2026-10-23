use leptos::prelude::*;
use leptos::task;

use crate::accounts::listed::{ListedAccount, OperatorLock};
use crate::accounts::service;
use crate::http::ApiError;

#[derive(Clone, Copy)]
pub(super) struct Roster {
    pub(super) accounts: RwSignal<Vec<ListedAccount>>,
    pub(super) next: RwSignal<Option<String>>,
    pub(super) loading: RwSignal<bool>,
    pub(super) refused: RwSignal<Option<ApiError>>,
    pub(super) locking: RwSignal<Option<ListedAccount>>,
}

impl Roster {
    pub(super) fn asked() -> Self {
        let roster = Self {
            accounts: RwSignal::new(Vec::new()),
            next: RwSignal::new(None),
            loading: RwSignal::new(false),
            refused: RwSignal::new(None),
            locking: RwSignal::new(None),
        };
        roster.load(None);

        roster
    }

    pub(super) fn load(self, after: Option<String>) {
        if self.loading.get_untracked() {
            return;
        }
        self.loading.set(true);
        task::spawn_local(async move {
            match service::list_accounts(after.as_deref()).await {
                Ok(page) => {
                    self.accounts.update(|listed| listed.extend(page.accounts));
                    self.next.set(page.next);
                }
                Err(failed) => self.refused.set(Some(failed)),
            }
            self.loading.set(false);
        });
    }

    pub(super) fn mark(self, id: &str, lock: Option<OperatorLock>) {
        self.accounts.update(|listed| {
            if let Some(account) = listed.iter_mut().find(|account| account.id == id) {
                account.lock = lock;
            }
        });
    }

    pub(super) fn unlock(self, id: String) {
        self.refused.set(None);
        task::spawn_local(async move {
            match service::unlock_account(&id).await {
                Ok(()) => self.mark(&id, None),
                Err(failed) => self.refused.set(Some(failed)),
            }
        });
    }
}
