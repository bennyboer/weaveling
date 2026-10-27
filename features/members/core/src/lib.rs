mod authorizer;
mod catalog;
mod error;
mod members;
mod members_core;
mod service;

#[cfg(test)]
mod members_tests;

pub use authorizer::MembersAuthorizer;
pub use catalog::{CatalogError, Membership, MembershipCatalog, Person, ProjectMembers};
pub use error::MembersCoreError;
pub use members::{
    KIND, Member, Members, MembersCommand, MembersError, MembersEvent, ProjectLink, Role,
};
pub use members_core::{ListedMember, MembersCore};
