use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use eventsourcing::AccountId;
use members_core::{
    CatalogError, Membership, MembershipCatalog, ProjectLink, ProjectMembers, Role,
};

type Kept = BTreeMap<String, ProjectMembers>;

#[derive(Debug, Default)]
pub struct InMemoryMembershipCatalog {
    kept: RwLock<Kept>,
}

impl InMemoryMembershipCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Kept> {
        self.kept.read().expect("membership catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Kept> {
        self.kept.write().expect("membership catalog lock poisoned")
    }
}

fn role_in(members: &ProjectMembers, account: &AccountId) -> Option<Role> {
    members
        .members
        .iter()
        .find(|member| member.account == *account)
        .map(|member| member.role)
}

#[async_trait]
impl MembershipCatalog for InMemoryMembershipCatalog {
    async fn remember(&self, members: &ProjectMembers) -> Result<(), CatalogError> {
        let mut kept = self.write();
        if kept
            .get(members.project.as_str())
            .is_none_or(|known| known.version < members.version)
        {
            kept.insert(members.project.to_string(), members.clone());
        }

        Ok(())
    }

    async fn role_of(
        &self,
        project: &ProjectLink,
        account: &AccountId,
    ) -> Result<Option<Role>, CatalogError> {
        Ok(self
            .read()
            .get(project.as_str())
            .and_then(|members| role_in(members, account)))
    }

    async fn memberships_of(&self, account: &AccountId) -> Result<Vec<Membership>, CatalogError> {
        Ok(self
            .read()
            .values()
            .filter_map(|members| {
                role_in(members, account).map(|role| Membership {
                    project: members.project.clone(),
                    role,
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::InMemoryMembershipCatalog;
    use crate::suite::Workbench;

    struct InMemory(InMemoryMembershipCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryMembershipCatalog;

        async fn setup() -> Self {
            Self(InMemoryMembershipCatalog::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
