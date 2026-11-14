use eventsourcing::AccountId;

use crate::asset::ProjectLink;
use crate::catalog::{Held, Upload};
use crate::uses::AssetUse;

/// How much storage the whole instance uses, and its limit, if it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceStorage {
    pub used: u64,
    pub most: Option<u64>,
}

/// How much storage an account's uploads use, wherever they went, and its limit.
/// The limit is the account's own, if an operator gave it one, or else the default for accounts.
/// Name and email are missing if the assets feature never heard them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountStorage {
    pub account: AccountId,
    pub name: Option<String>,
    pub email: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub own_limit: bool,
}

/// How much storage a project's assets use, its limit, and its owners.
/// The owners are listed so an operator knows whom to contact.
/// The name is missing if the assets feature never heard it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectStorage {
    pub project: ProjectLink,
    pub name: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub owners: Vec<ProjectOwner>,
}

/// An owner of a project, with their name and email.
/// Name and email are missing if the assets feature never heard them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectOwner {
    pub account: AccountId,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// One of an account's uploads, and whether the account may delete it.
/// Its own images it may always delete.
/// An upload into a project it may delete only while it has write access to that project.
/// `uses` says what the upload is used for. An upload in use cannot be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnUpload {
    pub upload: Upload,
    pub deletable: bool,
    pub uses: Vec<AssetUse>,
}

/// One page of a list, and the cursor for the next page.
/// The cursor is only set when the page is full, so a shorter page is the last one.
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
