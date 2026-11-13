use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use access::{AccessCatalog, AccessError, HearsAccess, InMemoryAccess, KeptAccess, Role};
use async_trait::async_trait;
use authorizing::Authorizer;
use eventsourcing::{AccountId, Version};
use scenes_core::ScenesCore;
use time::OffsetDateTime;
use transacting::Transaction;

/// Who may do what as the feature heard it, unless the list is down.
#[derive(Default)]
pub struct WhoMayDoWhat {
    kept: InMemoryAccess,
    pub down: AtomicBool,
}

impl WhoMayDoWhat {
    fn reachable(&self) -> Result<(), AccessError> {
        if self.down.load(Ordering::Acquire) {
            return Err(AccessError::Backend("the list is down".into()));
        }

        Ok(())
    }
}

#[async_trait]
impl AccessCatalog for WhoMayDoWhat {
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_role(transaction, project, account, version, role, at)
            .await
    }

    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_disbanded(transaction, project, version, at)
            .await
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_trash(transaction, project, version, in_the_trash)
            .await
    }

    async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_operator(transaction, account, version, operator, at)
            .await
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError> {
        self.kept
            .forget_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError> {
        self.reachable()?;
        self.kept.role_of(project, account).await
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.is_in_the_trash(project).await
    }

    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError> {
        self.reachable()?;
        self.kept.projects_of(account).await
    }

    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.share_a_project(one, other).await
    }

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.operates(account).await
    }
}

/// Who may do what as the scenes heard it, each word told newer than the last.
#[derive(Clone)]
pub struct Kept {
    pub who: Arc<WhoMayDoWhat>,
    pub access: KeptAccess,
    told: Arc<AtomicU64>,
}

impl Default for Kept {
    fn default() -> Self {
        let who = Arc::new(WhoMayDoWhat::default());

        Self {
            access: KeptAccess::new(who.clone()),
            who,
            told: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Kept {
    /// The authorizer the scenes are given where there are accounts.
    pub fn authorizer(&self) -> Arc<dyn Authorizer> {
        Arc::new(self.access.clone())
    }

    /// Tells the scenes an account's role in a project, or that it is a member no more.
    pub async fn give(
        &self,
        scenes: &ScenesCore,
        project: &str,
        account: &str,
        role: Option<Role>,
    ) {
        let version = self.told.fetch_add(1, Ordering::AcqRel) + 1;
        scenes
            .remember_role(
                None,
                project,
                &AccountId::from(account),
                Version::of(version),
                role,
            )
            .await
            .expect("remembering a role should succeed");
    }
}

/// Who may do what where nobody has been told anything, for scenes that allow everyone.
pub fn heard_of_nobody() -> KeptAccess {
    KeptAccess::new(Arc::new(InMemoryAccess::new()))
}
