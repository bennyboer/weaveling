use std::future::Future;
use std::mem;

use leptos::prelude::*;
use leptos::task;
use time::OffsetDateTime;

use crate::accounts::listed::{ListedAccount, OperatorLock};
use crate::accounts::service;
use crate::http::ApiError;
use crate::working_copy;
use crate::working_copy::{Versioned, WorkingCopies};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Intent {
    Lock {
        typed: String,
        since: OffsetDateTime,
    },
    Unlock,
    Appoint,
    Dismiss,
}

#[derive(Clone, Copy)]
pub(super) struct Roster {
    accounts: WorkingCopies<String, ListedAccount, Intent>,
    pub(super) next: RwSignal<Option<String>>,
    pub(super) loading: RwSignal<bool>,
    pub(super) refused: RwSignal<Option<ApiError>>,
    pub(super) locking: RwSignal<Option<ListedAccount>>,
    pub(super) changing_role: RwSignal<Option<ListedAccount>>,
}

impl Roster {
    pub(super) fn asked() -> Self {
        let refused = RwSignal::new(None);
        let accounts = WorkingCopies::new(
            move |id: String, on: ListedAccount, intent: Intent| async move {
                match &intent {
                    Intent::Lock { typed, .. } => {
                        service::lock_account(&id, typed, on.version).await?
                    }
                    Intent::Unlock => service::unlock_account(&id, on.version).await?,
                    Intent::Appoint => service::appoint_operator(&id, on.version).await?,
                    Intent::Dismiss => service::dismiss_operator(&id, on.version).await?,
                }

                Ok(answered(
                    on,
                    &intent,
                    service::listed_account(&id).await.ok(),
                ))
            },
            move |id: String| async move { service::listed_account(&id).await },
            move |failure| _ = refused.try_set(Some(failure)),
        );
        let roster = Self {
            accounts,
            next: RwSignal::new(None),
            loading: RwSignal::new(false),
            refused,
            locking: RwSignal::new(None),
            changing_role: RwSignal::new(None),
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
                    let mut listed = self.accounts.confirmed_all();
                    listed.extend(
                        page.accounts
                            .into_iter()
                            .map(|account| (account.id.clone(), account)),
                    );
                    self.accounts.take_in_all(listed);
                    _ = self.next.try_set(page.next);
                }
                Err(failed) => _ = self.refused.try_set(Some(failed)),
            }
            _ = self.loading.try_set(false);
        });
    }

    pub(super) fn shown(self) -> Vec<ListedAccount> {
        self.accounts.shown()
    }

    pub(super) fn intended(
        self,
        id: &String,
        intent: Intent,
    ) -> impl Future<Output = bool> + use<> {
        let landing = (!untrack(|| self.accounts.unanswered_on(id))).then(|| {
            self.refused.set(None);
            self.accounts.intended(id, intent)
        });

        async move {
            let Some(landing) = landing else {
                return false;
            };
            landing
                .await
                .map_err(|failure| _ = self.refused.try_set(Some(failure)))
                .is_ok()
        }
    }

    pub(super) fn changing(self, id: &String) -> bool {
        self.accounts.unanswered_on(id)
    }

    pub(super) fn unlock(self, id: String) {
        self.refused.set(None);
        self.accounts.intend(&id, Intent::Unlock);
    }
}

impl Versioned for ListedAccount {
    fn version(&self) -> u64 {
        self.version
    }
}

impl working_copy::Intent<ListedAccount> for Intent {
    fn lay_over(&self, account: &mut ListedAccount) {
        match self {
            Self::Lock { typed, since } => {
                account.lock = Some(OperatorLock {
                    reason: typed.trim().to_owned(),
                    since: *since,
                })
            }
            Self::Unlock => account.lock = None,
            Self::Appoint => account.operator = true,
            Self::Dismiss => account.operator = false,
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        (mem::discriminant(self) == mem::discriminant(earlier)).then(|| self.clone())
    }
}

pub(super) fn now() -> OffsetDateTime {
    let millis = js_sys::Date::now() as i64;

    OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

fn answered(on: ListedAccount, intent: &Intent, read: Option<ListedAccount>) -> ListedAccount {
    match read {
        Some(read) if read.version > on.version => read,
        _ => {
            let mut answered = on;
            working_copy::Intent::lay_over(intent, &mut answered);
            answered.version += 1;

            answered
        }
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::working_copy::Intent as _;

    fn grace(operator: bool, lock: Option<OperatorLock>) -> ListedAccount {
        ListedAccount {
            id: "account_2".to_owned(),
            version: 3,
            name: "Grace Hopper".to_owned(),
            email: "grace@example.com".to_owned(),
            verified: true,
            operator,
            signed_up_at: datetime!(2026-10-23 08:00 UTC),
            lock,
        }
    }

    fn a_lock_for(typed: &str) -> Intent {
        Intent::Lock {
            typed: typed.to_owned(),
            since: datetime!(2026-11-05 09:30 UTC),
        }
    }

    fn spam() -> OperatorLock {
        OperatorLock {
            reason: "Sent spam.".to_owned(),
            since: datetime!(2026-11-05 09:30 UTC),
        }
    }

    fn shown(mut on: ListedAccount, intent: &Intent) -> ListedAccount {
        intent.lay_over(&mut on);
        on
    }

    #[test]
    fn a_lock_on_its_way_shows_the_account_locked_for_the_reason_trimmed() {
        // when: a lock is laid over an account not locked
        let locked = shown(grace(false, None), &a_lock_for("  Sent spam. "));

        // then: it shows locked since then, for the reason without its spaces
        assert_eq!(locked.lock, Some(spam()));
    }

    #[test]
    fn an_unlock_on_its_way_shows_the_account_unlocked() {
        // expect: an unlock laid over a locked account shows it unlocked
        assert_eq!(
            shown(grace(false, Some(spam())), &Intent::Unlock).lock,
            None
        );
    }

    #[test]
    fn a_role_given_or_taken_shows_at_once() {
        // expect: appointing shows an operator
        assert!(shown(grace(false, None), &Intent::Appoint).operator);
        // and: dismissing shows one no longer
        assert!(!shown(grace(true, None), &Intent::Dismiss).operator);
    }

    #[test]
    fn only_the_same_change_waiting_is_merged_and_the_newer_one_wins() {
        // expect: a lock waiting after a lock is sent as the newer
        assert_eq!(
            a_lock_for("Second.").merged_after(&a_lock_for("First.")),
            Some(a_lock_for("Second."))
        );
        // and: a change of another kind waits its own turn
        assert_eq!(Intent::Unlock.merged_after(&a_lock_for("First.")), None);
        assert_eq!(Intent::Dismiss.merged_after(&Intent::Appoint), None);
    }

    #[test]
    fn a_change_landed_stands_as_read_again_when_that_is_newer() {
        // given: the account as read again after appointing it, at a newer version
        let mut read = grace(true, None);
        read.version = 4;

        // expect: the answer is what was read
        assert_eq!(
            answered(grace(false, None), &Intent::Appoint, Some(read.clone())),
            read
        );
    }

    #[test]
    fn a_change_landed_but_not_read_newer_stands_one_version_on_with_the_change_in_it() {
        // given: an account at version three, read again no newer or not at all
        for read in [Some(grace(false, None)), None] {
            // when: appointing it is answered
            let answer = answered(grace(false, None), &Intent::Appoint, read);

            // then: it is an operator, one version on
            assert!(answer.operator);
            assert_eq!(
                answer.version, 4,
                "an answer no newer than what it was sent on would be ignored and the change would seem undone"
            );
        }
    }
}
