use std::sync::Arc;

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use eventsourcing::{AccountId, Agent, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::{AccessCatalog, AccessError, Role};

/// Answers who may do what, from the access updates the feature has received itself.
///
/// It is the `Authorizer` backed by the feature's own `AccessCatalog`.
/// Each update is written in the listener's transaction.
/// Once that transaction has committed, everyone who follows the project live is told that its access changed.
#[derive(Clone)]
pub struct KeptAccess {
    catalog: Arc<dyn AccessCatalog>,
    changes: AccessChanges,
}

fn unreachable(failure: AccessError) -> AuthorizerError {
    AuthorizerError::Unreachable(Box::new(failure))
}

impl KeptAccess {
    pub fn new(catalog: Arc<dyn AccessCatalog>) -> Self {
        Self {
            catalog,
            changes: AccessChanges::new(),
        }
    }

    fn changed_once_committed(&self, transaction: &mut Transaction, project: &str) {
        let changes = self.changes.clone();
        let project = project.to_owned();
        transaction.after_commit(async move { changes.tell(&project) });
    }

    pub async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.catalog
            .remember_role(transaction, project, account, version, role, at)
            .await?;
        self.changed_once_committed(transaction, project);

        Ok(())
    }

    pub async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.catalog
            .remember_disbanded(transaction, project, version, at)
            .await?;
        self.changed_once_committed(transaction, project);

        Ok(())
    }

    pub async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AccessError> {
        self.catalog
            .remember_trash(transaction, project, version, in_the_trash)
            .await?;
        self.changed_once_committed(transaction, project);

        Ok(())
    }

    pub async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.catalog
            .remember_operator(transaction, account, version, operator, at)
            .await
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError> {
        self.catalog
            .forget_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    /// Whether two accounts are both members of at least one project that is neither disbanded nor in the trash.
    pub async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError> {
        self.catalog.share_a_project(one, other).await
    }

    /// The account ids of a project's owners, unless the project is disbanded.
    pub async fn owners_of(&self, project: &str) -> Result<Vec<AccountId>, AccessError> {
        self.catalog.owners_of(project).await
    }
}

#[async_trait]
impl Authorizer for KeptAccess {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        let account = match agent {
            Agent::System => return Ok(true),
            Agent::Local => return Ok(false),
            Agent::Account(account) => account,
        };
        let permitted = self
            .catalog
            .role_of(project, account)
            .await
            .map_err(unreachable)?
            .is_some_and(|role| role.permits(act));
        if !permitted || act == Act::Restore {
            return Ok(permitted);
        }

        Ok(!self
            .catalog
            .is_in_the_trash(project)
            .await
            .map_err(unreachable)?)
    }

    async fn readable_by(&self, agent: &Agent) -> Result<Readable, AuthorizerError> {
        match agent {
            Agent::System => Ok(Readable::Everything),
            Agent::Local => Ok(Readable::Only(Default::default())),
            Agent::Account(account) => Ok(Readable::Only(
                self.catalog
                    .projects_of(account)
                    .await
                    .map_err(unreachable)?
                    .into_iter()
                    .collect(),
            )),
        }
    }

    async fn operates(&self, agent: &Agent) -> Result<bool, AuthorizerError> {
        match agent {
            Agent::System => Ok(true),
            Agent::Local => Ok(false),
            Agent::Account(account) => self.catalog.operates(account).await.map_err(unreachable),
        }
    }

    fn access_changed(&self, project: &str) {
        self.changes.tell(project);
    }

    fn watch_access(&self, project: &str) -> AccessWatch {
        self.changes.watch(project)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::time::Duration;

    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::InMemoryAccess;

    const THE_LOOM: &str = "project_loom";
    const THE_WEFT: &str = "project_weft";

    fn ada() -> AccountId {
        AccountId::from("account_ada")
    }

    fn bo() -> AccountId {
        AccountId::from("account_bo")
    }

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
    }

    struct Kept {
        access: KeptAccess,
        transactions: InMemoryTransactions,
    }

    impl Kept {
        fn new() -> Self {
            Self {
                access: KeptAccess::new(Arc::new(InMemoryAccess::new())),
                transactions: InMemoryTransactions::new(),
            }
        }

        async fn begun(&self) -> Transaction {
            self.transactions
                .begin()
                .await
                .expect("beginning should succeed")
        }

        async fn role(&self, project: &str, account: AccountId, version: u64, role: Option<Role>) {
            let mut transaction = self.begun().await;
            self.access
                .remember_role(
                    &mut transaction,
                    project,
                    &account,
                    Version::of(version),
                    role,
                    at(1_000),
                )
                .await
                .expect("remembering should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }

        async fn trash(&self, project: &str, version: u64, in_the_trash: bool) {
            let mut transaction = self.begun().await;
            self.access
                .remember_trash(
                    &mut transaction,
                    project,
                    Version::of(version),
                    in_the_trash,
                )
                .await
                .expect("remembering should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }

        async fn operator(&self, account: AccountId, version: u64, operator: bool) {
            let mut transaction = self.begun().await;
            self.access
                .remember_operator(
                    &mut transaction,
                    &account,
                    Version::of(version),
                    operator,
                    at(1_000),
                )
                .await
                .expect("remembering should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }

        async fn may(&self, agent: &Agent, act: Act, project: &str) -> bool {
            self.access
                .may(agent, act, project)
                .await
                .expect("asking should succeed")
        }
    }

    #[tokio::test]
    async fn a_member_may_do_what_their_role_permits_in_their_project_alone() {
        // given: Ada owning the loom, Bo reading it
        let kept = Kept::new();
        kept.role(THE_LOOM, ada(), 1, Some(Role::Owner)).await;
        kept.role(THE_LOOM, bo(), 2, Some(Role::Reader)).await;

        // expect: Ada may do everything there, Bo read alone, and neither anything in the weft
        for act in [Act::Read, Act::Write, Act::Manage, Act::Restore] {
            assert!(
                kept.may(&Agent::Account(ada()), act, THE_LOOM).await,
                "{act:?}"
            );
            assert_eq!(
                kept.may(&Agent::Account(bo()), act, THE_LOOM).await,
                act == Act::Read,
                "{act:?}"
            );
            assert!(
                !kept.may(&Agent::Account(ada()), act, THE_WEFT).await,
                "{act:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_project_in_the_trash_may_only_be_restored() {
        // given: Ada owning the loom, which is in the trash
        let kept = Kept::new();
        kept.role(THE_LOOM, ada(), 1, Some(Role::Owner)).await;
        kept.trash(THE_LOOM, 4, true).await;

        // expect: she may restore it, and do nothing else in it
        let ada = Agent::Account(ada());
        assert!(kept.may(&ada, Act::Restore, THE_LOOM).await);
        for act in [Act::Read, Act::Write, Act::Manage] {
            assert!(!kept.may(&ada, act, THE_LOOM).await, "{act:?}");
        }
        // and: it is not hers to read
        assert_eq!(
            kept.access
                .readable_by(&ada)
                .await
                .expect("asking should succeed"),
            Readable::Only(BTreeSet::new())
        );
    }

    #[tokio::test]
    async fn the_system_may_do_everything_and_nobody_signed_in_nothing() {
        // given: Ada owning the loom
        let kept = Kept::new();
        kept.role(THE_LOOM, ada(), 1, Some(Role::Owner)).await;

        // expect: the system may do and read everything, and runs the instance
        for act in [Act::Read, Act::Write, Act::Manage, Act::Restore] {
            assert!(kept.may(&Agent::System, act, THE_LOOM).await, "{act:?}");
            assert!(!kept.may(&Agent::Local, act, THE_LOOM).await, "{act:?}");
        }
        assert_eq!(
            kept.access
                .readable_by(&Agent::System)
                .await
                .expect("asking should succeed"),
            Readable::Everything
        );
        // and: nobody signed in reads nothing and runs nothing
        assert_eq!(
            kept.access
                .readable_by(&Agent::Local)
                .await
                .expect("asking should succeed"),
            Readable::Only(BTreeSet::new())
        );
        assert!(
            kept.access
                .operates(&Agent::System)
                .await
                .expect("asking should succeed")
        );
        assert!(
            !kept
                .access
                .operates(&Agent::Local)
                .await
                .expect("asking should succeed")
        );
    }

    #[tokio::test]
    async fn an_account_reads_the_projects_it_is_a_member_of() {
        // given: Ada owning the loom and reading the weft
        let kept = Kept::new();
        kept.role(THE_LOOM, ada(), 1, Some(Role::Owner)).await;
        kept.role(THE_WEFT, ada(), 3, Some(Role::Reader)).await;

        // expect: she reads both, and Bo neither
        assert_eq!(
            kept.access
                .readable_by(&Agent::Account(ada()))
                .await
                .expect("asking should succeed"),
            Readable::Only(BTreeSet::from([THE_LOOM.to_owned(), THE_WEFT.to_owned()]))
        );
        assert_eq!(
            kept.access
                .readable_by(&Agent::Account(bo()))
                .await
                .expect("asking should succeed"),
            Readable::Only(BTreeSet::new())
        );
    }

    #[tokio::test]
    async fn an_account_runs_the_instance_as_last_told() {
        // given: Ada became an operator, and Bo became one and stopped
        let kept = Kept::new();
        kept.operator(ada(), 2, true).await;
        kept.operator(bo(), 2, true).await;
        kept.operator(bo(), 3, false).await;

        // expect: Ada runs it, and Bo not
        assert!(
            kept.access
                .operates(&Agent::Account(ada()))
                .await
                .expect("asking should succeed")
        );
        assert!(
            !kept
                .access
                .operates(&Agent::Account(bo()))
                .await
                .expect("asking should succeed")
        );
    }

    #[tokio::test]
    async fn whoever_follows_a_project_is_told_once_its_access_changed_is_committed() {
        // given: someone following the loom live
        let kept = Kept::new();
        let mut watch = kept.access.watch_access(THE_LOOM);

        // when: Bo's leaving it is remembered, but not yet committed
        let mut transaction = kept.begun().await;
        kept.access
            .remember_role(
                &mut transaction,
                THE_LOOM,
                &bo(),
                Version::of(3),
                None,
                at(1_000),
            )
            .await
            .expect("remembering should succeed");

        // then: nothing is told yet
        assert!(
            tokio::time::timeout(Duration::from_millis(50), watch.changed())
                .await
                .is_err(),
            "a change not committed may still be rolled back"
        );
        // when: it is committed
        transaction
            .finish()
            .await
            .expect("finishing should succeed");
        // then: the follower is told
        tokio::time::timeout(Duration::from_secs(1), watch.changed())
            .await
            .expect("the change is told once committed");
    }
}
