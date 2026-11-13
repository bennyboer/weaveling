use eventsourcing::AccountId;

use crate::asset::ProjectLink;
use crate::catalog::{Held, Upload};

/// What the instance holds of every asset, and the most it may.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceStorage {
    pub used: u64,
    pub most: Option<u64>,
}

/// What an account's uploads take, wherever they went, and the most they may: its own limit, given by the operator, or the one of every account; with its name and address, where the assets heard them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountStorage {
    pub account: AccountId,
    pub name: Option<String>,
    pub email: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub own_limit: bool,
}

/// What a project's assets take, the most they may, and its owners, to speak to about it; with its name, where the assets heard it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectStorage {
    pub project: ProjectLink,
    pub name: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub owners: Vec<AccountId>,
}

/// One of an account's uploads, and whether it may delete it: its own always, one in a project where it may write there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnUpload {
    pub upload: Upload,
    pub deletable: bool,
}

/// A page, and where the next goes on from where there is one: after the last listed, where the page is full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed<T, Mark = Held> {
    pub listed: Vec<T>,
    pub next: Option<Mark>,
}

impl<T, Mark> Listed<T, Mark> {
    pub(crate) fn paged(listed: Vec<T>, last: Option<Mark>, at_most: u64) -> Self {
        let full = u64::try_from(listed.len()).is_ok_and(|count| count >= at_most);

        Self {
            listed,
            next: last.filter(|_| full),
        }
    }
}
