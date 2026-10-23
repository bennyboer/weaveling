use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use invitations_core::{InvitationId, InvitationLinks, InvitationTokenDigest, LinksError};

#[derive(Debug, Default)]
pub struct InMemoryInvitationLinks {
    kept: Mutex<HashMap<InvitationTokenDigest, InvitationId>>,
}

impl InMemoryInvitationLinks {
    fn kept(&self) -> MutexGuard<'_, HashMap<InvitationTokenDigest, InvitationId>> {
        self.kept
            .lock()
            .expect("the invitation links are not poisoned")
    }
}

#[async_trait]
impl InvitationLinks for InMemoryInvitationLinks {
    async fn keep(
        &self,
        digest: &InvitationTokenDigest,
        invitation: &InvitationId,
    ) -> Result<(), LinksError> {
        self.kept().insert(digest.clone(), *invitation);

        Ok(())
    }

    async fn invitation_of(
        &self,
        digest: &InvitationTokenDigest,
    ) -> Result<Option<InvitationId>, LinksError> {
        Ok(self.kept().get(digest).copied())
    }

    async fn forget(&self, invitation: &InvitationId) -> Result<(), LinksError> {
        self.kept().retain(|_, kept| kept != invitation);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::InMemoryInvitationLinks;
    use crate::suite::Workbench;

    struct InMemory(InMemoryInvitationLinks);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryInvitationLinks;

        async fn setup() -> Self {
            Self(InMemoryInvitationLinks::default())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
