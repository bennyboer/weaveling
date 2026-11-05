use leptos::prelude::*;
use leptos::task;

use crate::act::{Act, Here};
use crate::http::ApiError;
use crate::invitations::listed::{ListedInvitation, Scope};
use crate::invitations::service;
use crate::invitations::service::InvitedInto;
use crate::members::listed::Role;
use crate::working_copy;
use crate::working_copy::{Versioned, WorkingCopies};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Intent {
    Withdraw,
}

#[derive(Clone, Copy)]
pub struct Invited {
    pub(super) into: StoredValue<InvitedInto>,
    pub(super) invitations: WorkingCopies<String, ListedInvitation, Intent>,
    pub(super) next: RwSignal<Option<String>>,
    pub(super) loading: RwSignal<bool>,
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
        let refused = RwSignal::new(None);
        let invitations = WorkingCopies::new(
            move |id: String, on: ListedInvitation, intent: Intent| async move {
                match intent {
                    Intent::Withdraw => service::withdraw(&id, on.version).await?,
                }

                Ok(answered(on, &intent))
            },
            move |id: String| {
                let into = into.get_value();

                async move {
                    service::list_invitations(&into, None)
                        .await?
                        .invitations
                        .into_iter()
                        .find(|invitation| invitation.id == id)
                        .map(|invitation| held(invitation).1)
                        .ok_or(ApiError::NotFound("invitation"))
                }
            },
            move |failure| _ = refused.try_set(Some(failure)),
        );
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
                Inviting::Again(id) => untrack(|| invitations.shown_one(id))
                    .map(|held| (held.email.clone(), held.scope.clone())),
            };

            async move {
                let Some((invited, scope)) = asked else {
                    return;
                };
                match service::invite(&invited, &scope).await {
                    Ok(invitation) => {
                        let mut listed = vec![held(invitation)];
                        listed.extend(still_listed(invitations));
                        invitations.take_in_all(listed);
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
            refused,
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
                    let mut listed = still_listed(self.invitations);
                    listed.extend(page.invitations.into_iter().map(held));
                    self.invitations.take_in_all(listed);
                    self.next.set(page.next);
                }
                Err(failed) => self.refused.set(Some(failed)),
            }
            self.loading.set(false);
        });
    }

    pub(super) fn withdraw(self, id: String) {
        let withdrawn_or_gone =
            untrack(|| !self.invitations.is_listed(&id) || self.invitations.unanswered_on(&id));
        if withdrawn_or_gone {
            return;
        }
        self.refused.set(None);
        let Some(landed) = self.invitations.intended(&id, Intent::Withdraw) else {
            return;
        };
        task::spawn_local(async move {
            if landed.await {
                self.invitations.forget(&id);
            }
        });
    }
}

impl Versioned for ListedInvitation {
    fn version(&self) -> u64 {
        self.version
    }
}

impl working_copy::Intent<ListedInvitation> for Intent {
    fn lay_over(&self, _: &mut ListedInvitation) {}

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        (self == earlier).then(|| self.clone())
    }
}

fn held(invitation: ListedInvitation) -> (String, ListedInvitation) {
    (invitation.id.clone(), invitation)
}

fn answered(on: ListedInvitation, intent: &Intent) -> ListedInvitation {
    let mut answered = on;
    working_copy::Intent::lay_over(intent, &mut answered);
    answered.version += 1;

    answered
}

fn still_listed(
    invitations: WorkingCopies<String, ListedInvitation, Intent>,
) -> Vec<(String, ListedInvitation)> {
    invitations.confirmed_all()
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

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::invitations::listed::Status;
    use crate::working_copy::Intent as _;

    fn an_open_invitation() -> ListedInvitation {
        held(ListedInvitation {
            id: "invitation_1".to_owned(),
            version: 1,
            email: "grace@example.com".to_owned(),
            issued_at: datetime!(2026-10-23 08:00 UTC),
            until: datetime!(2026-10-30 08:00 UTC),
            status: Status::Open,
            scope: Scope::Instance,
        })
        .1
    }

    #[test]
    fn a_withdrawal_on_its_way_changes_nothing_shown() {
        // given: an open invitation
        let open = an_open_invitation();

        // when: its withdrawal is laid over it
        let mut shown = open.clone();
        Intent::Withdraw.lay_over(&mut shown);

        // then: it shows as it was
        assert_eq!(shown, open);
    }

    #[test]
    fn a_withdrawal_answered_stands_one_version_after_the_one_it_was_sent_on() {
        // given: an open invitation at version one
        let open = an_open_invitation();

        // when: its withdrawal is answered
        let answered = answered(open, &Intent::Withdraw);

        // then: it stands at the next version
        assert_eq!(answered.version(), 2);
    }

    #[test]
    fn a_second_withdrawal_waiting_is_sent_as_one() {
        // expect: two withdrawals in a row merge into one
        assert_eq!(
            Intent::Withdraw.merged_after(&Intent::Withdraw),
            Some(Intent::Withdraw)
        );
    }
}
