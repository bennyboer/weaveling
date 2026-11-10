use accounts_contract::{AccountPageDTO, AccountSummaryDTO, deletion_asked_by, lock_kind};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::accounts::PendingDeletion;
use crate::http::ApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedAccount {
    pub id: String,
    pub version: u64,
    pub name: String,
    pub email: String,
    pub verified: bool,
    pub operator: bool,
    pub signed_up_at: OffsetDateTime,
    pub lock: Option<OperatorLock>,
    pub second_factor: bool,
    pub deletion: Option<PendingDeletion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorLock {
    pub reason: String,
    pub since: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPage {
    pub accounts: Vec<ListedAccount>,
    pub next: Option<String>,
}

pub(crate) fn moment(written: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(written, &Rfc3339).map_err(|_| ApiError::Unexpected)
}

fn listed(summary: AccountSummaryDTO) -> Result<ListedAccount, ApiError> {
    let lock = summary
        .locks
        .into_iter()
        .find(|lock| lock.kind == lock_kind::OPERATOR)
        .map(|lock| {
            Ok::<_, ApiError>(OperatorLock {
                reason: lock.reason,
                since: moment(&lock.since)?,
            })
        })
        .transpose()?;

    Ok(ListedAccount {
        id: summary.id,
        version: summary.version,
        name: summary.name,
        email: summary.email,
        verified: summary.verified,
        operator: summary.operator,
        signed_up_at: moment(&summary.signed_up_at)?,
        lock,
        second_factor: summary.second_factor,
        deletion: summary
            .deletion
            .map(|deletion| {
                Ok::<_, ApiError>(PendingDeletion {
                    until: moment(&deletion.until)?,
                    by_the_operator: deletion.by == deletion_asked_by::OPERATOR,
                })
            })
            .transpose()?,
    })
}

impl AccountPage {
    pub fn of(page: AccountPageDTO) -> Result<Self, ApiError> {
        Ok(Self {
            accounts: page
                .accounts
                .into_iter()
                .map(listed)
                .collect::<Result<_, _>>()?,
            next: page.next,
        })
    }
}

#[cfg(test)]
mod tests {
    use accounts_contract::LockDTO;
    use time::macros::datetime;

    use super::*;

    fn an_account_of_ada(locks: Vec<LockDTO>) -> AccountSummaryDTO {
        AccountSummaryDTO {
            second_factor: false,
            id: "account_1".to_owned(),
            version: 4,
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
            verified: true,
            operator: false,
            signed_up_at: "2026-10-23T08:00:00Z".to_owned(),
            locks,
            deletion: None,
        }
    }

    #[test]
    fn an_account_to_be_deleted_is_listed_with_its_day_and_who_asked() {
        // when: an account the operator marked is read
        let page = AccountPage::of(AccountPageDTO {
            accounts: vec![AccountSummaryDTO {
                deletion: Some(accounts_contract::DeletionDTO {
                    until: "2026-12-09T10:00:00Z".to_owned(),
                    by: deletion_asked_by::OPERATOR.to_owned(),
                }),
                ..an_account_of_ada(Vec::new())
            }],
            next: None,
        })
        .expect("a page the server sends is readable");

        // then: it is listed to be deleted on its day, as the operator asked
        assert_eq!(
            page.accounts[0].deletion,
            Some(PendingDeletion {
                until: datetime!(2026-12-09 10:00 UTC),
                by_the_operator: true,
            })
        );
    }

    #[test]
    fn an_account_is_listed_as_the_server_told_it() {
        // when: a page of one account the operator locked is read
        let page = AccountPage::of(AccountPageDTO {
            accounts: vec![an_account_of_ada(vec![LockDTO {
                kind: "operator".to_owned(),
                reason: "Sent spam.".to_owned(),
                since: "2026-10-23T09:30:00Z".to_owned(),
            }])],
            next: Some("account_1".to_owned()),
        })
        .expect("a page the server sends is readable");

        // then: it lists the account, its lock and the next page as told
        assert_eq!(
            page,
            AccountPage {
                accounts: vec![ListedAccount {
                    id: "account_1".to_owned(),
                    version: 4,
                    name: "Ada".to_owned(),
                    email: "ada@example.com".to_owned(),
                    verified: true,
                    operator: false,
                    signed_up_at: datetime!(2026-10-23 08:00 UTC),
                    lock: Some(OperatorLock {
                        reason: "Sent spam.".to_owned(),
                        since: datetime!(2026-10-23 09:30 UTC),
                    }),
                    second_factor: false,
                    deletion: None,
                }],
                next: Some("account_1".to_owned()),
            }
        );
    }

    #[test]
    fn only_the_operators_lock_is_shown_as_one() {
        // when: a page of one account pending deletion is read
        let page = AccountPage::of(AccountPageDTO {
            accounts: vec![an_account_of_ada(vec![LockDTO {
                kind: "pending-deletion".to_owned(),
                reason: "Asked to be deleted.".to_owned(),
                since: "2026-10-23T09:30:00Z".to_owned(),
            }])],
            next: None,
        })
        .expect("a page the server sends is readable");

        // then: the account is listed with no lock
        assert_eq!(page.accounts[0].lock, None);
    }

    #[test]
    fn a_moment_that_is_no_moment_is_unexpected() {
        // given: an account signed up at no moment
        let mut broken = an_account_of_ada(vec![]);
        broken.signed_up_at = "yesterday".to_owned();

        // then: a page of it is unexpected
        assert_eq!(
            AccountPage::of(AccountPageDTO {
                accounts: vec![broken],
                next: None,
            }),
            Err(ApiError::Unexpected)
        );
    }
}
