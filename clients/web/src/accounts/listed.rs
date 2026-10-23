use accounts_contract::{AccountPageDTO, AccountSummaryDTO, lock_kind};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http::ApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedAccount {
    pub id: String,
    pub name: String,
    pub email: String,
    pub verified: bool,
    pub operator: bool,
    pub signed_up_at: OffsetDateTime,
    pub lock: Option<OperatorLock>,
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

fn moment(written: &str) -> Result<OffsetDateTime, ApiError> {
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
        name: summary.name,
        email: summary.email,
        verified: summary.verified,
        operator: summary.operator,
        signed_up_at: moment(&summary.signed_up_at)?,
        lock,
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

    fn ada(locks: Vec<LockDTO>) -> AccountSummaryDTO {
        AccountSummaryDTO {
            id: "account_1".to_owned(),
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
            verified: true,
            operator: false,
            signed_up_at: "2026-10-23T08:00:00Z".to_owned(),
            locks,
        }
    }

    #[test]
    fn an_account_is_listed_as_the_server_told_it() {
        let page = AccountPage::of(AccountPageDTO {
            accounts: vec![ada(vec![LockDTO {
                kind: "operator".to_owned(),
                reason: "Sent spam.".to_owned(),
                since: "2026-10-23T09:30:00Z".to_owned(),
            }])],
            next: Some("account_1".to_owned()),
        })
        .expect("a page the server sends is readable");

        assert_eq!(
            page,
            AccountPage {
                accounts: vec![ListedAccount {
                    id: "account_1".to_owned(),
                    name: "Ada".to_owned(),
                    email: "ada@example.com".to_owned(),
                    verified: true,
                    operator: false,
                    signed_up_at: datetime!(2026-10-23 08:00 UTC),
                    lock: Some(OperatorLock {
                        reason: "Sent spam.".to_owned(),
                        since: datetime!(2026-10-23 09:30 UTC),
                    }),
                }],
                next: Some("account_1".to_owned()),
            }
        );
    }

    #[test]
    fn only_the_operators_lock_is_shown_as_one() {
        let page = AccountPage::of(AccountPageDTO {
            accounts: vec![ada(vec![LockDTO {
                kind: "pending-deletion".to_owned(),
                reason: "Asked to be deleted.".to_owned(),
                since: "2026-10-23T09:30:00Z".to_owned(),
            }])],
            next: None,
        })
        .expect("a page the server sends is readable");

        assert_eq!(page.accounts[0].lock, None);
    }

    #[test]
    fn a_moment_that_is_no_moment_is_unexpected() {
        let mut broken = ada(vec![]);
        broken.signed_up_at = "yesterday".to_owned();

        assert_eq!(
            AccountPage::of(AccountPageDTO {
                accounts: vec![broken],
                next: None,
            }),
            Err(ApiError::Unexpected)
        );
    }
}
