use std::ops::Deref;

use eventsourcing::{AccountId, Version};
use members_core::{
    CatalogError, Member, Membership, MembershipCatalog, Person, ProjectLink, ProjectMembers, Role,
};
use transacting::{Transaction, Transactions, Tried};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: MembershipCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Store> {
        Committing {
            catalog: self.store(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

pub struct Committing<'a, C> {
    catalog: &'a C,
    transactions: &'a dyn Transactions,
}

fn unreachable(failure: transacting::Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

impl<C: MembershipCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    pub async fn remember(&self, members: &ProjectMembers) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, members)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn remember_trash(
        &self,
        project: &str,
        version: u64,
        in_the_trash: bool,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_trash(
                    transaction,
                    &ProjectLink::from(project),
                    Version::of(version),
                    in_the_trash,
                )
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_trash(&self, project: &str) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget_trash(transaction, &ProjectLink::from(project))
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn remember_person(&self, person: &Person) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_person(transaction, person)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }
}

impl<C> Deref for Committing<'_, C> {
    type Target = C;

    fn deref(&self) -> &C {
        self.catalog
    }
}

fn ada() -> AccountId {
    AccountId::from("account_ada")
}

fn grace() -> AccountId {
    AccountId::from("account_grace")
}

fn hedy() -> AccountId {
    AccountId::from("account_hedy")
}

fn a_project(named: &str) -> ProjectLink {
    ProjectLink::from(named)
}

pub fn members_of(project: &str, version: u64, members: &[(AccountId, Role)]) -> ProjectMembers {
    ProjectMembers {
        project: a_project(project),
        version: Version::of(version),
        members: members
            .iter()
            .map(|(account, role)| Member {
                account: account.clone(),
                role: *role,
            })
            .collect(),
    }
}

async fn remembered(catalog: &Committing<'_, impl MembershipCatalog>, members: &ProjectMembers) {
    catalog
        .remember(members)
        .await
        .expect("remembering should succeed");
}

async fn role(
    catalog: &Committing<'_, impl MembershipCatalog>,
    project: &str,
    account: &AccountId,
) -> Option<Role> {
    catalog
        .role_of(&a_project(project), account)
        .await
        .expect("asking for a role should succeed")
}

async fn memberships(
    catalog: &Committing<'_, impl MembershipCatalog>,
    account: &AccountId,
) -> Vec<Membership> {
    catalog
        .memberships_of(account)
        .await
        .expect("listing memberships should succeed")
}

fn person(account: AccountId, name: &str, email: &str) -> Person {
    Person {
        account,
        name: name.to_owned(),
        email: email.to_owned(),
    }
}

async fn known(catalog: &Committing<'_, impl MembershipCatalog>, person: &Person) {
    catalog
        .remember_person(person)
        .await
        .expect("remembering a person should succeed");
}

async fn people(
    catalog: &Committing<'_, impl MembershipCatalog>,
    accounts: &[AccountId],
) -> Vec<Person> {
    catalog
        .people(accounts)
        .await
        .expect("asking for people should succeed")
}

pub async fn an_empty_catalog_knows_nobody_in_any_project(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    assert_eq!(role(catalog, "project_1", &ada()).await, None);
    assert!(memberships(catalog, &ada()).await.is_empty());
    assert!(people(catalog, &[ada()]).await.is_empty());
}

pub async fn the_people_asked_for_are_told_by_account_and_no_others(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    known(catalog, &person(hedy(), "Hedy Lamarr", "hedy@example.com")).await;
    known(
        catalog,
        &person(grace(), "Grace Hopper", "grace@example.com"),
    )
    .await;
    known(catalog, &person(ada(), "Ada Lovelace", "ada@example.com")).await;

    assert_eq!(
        people(
            catalog,
            &[grace(), AccountId::from("account_nobody"), ada()]
        )
        .await,
        vec![
            person(ada(), "Ada Lovelace", "ada@example.com"),
            person(grace(), "Grace Hopper", "grace@example.com"),
        ]
    );
    assert!(people(catalog, &[]).await.is_empty());
}

pub async fn a_person_told_again_is_known_as_last_told(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    known(catalog, &person(ada(), "Ada", "ada@example.com")).await;

    known(
        catalog,
        &person(ada(), "Ada Lovelace", "lovelace@example.com"),
    )
    .await;

    assert_eq!(
        people(catalog, &[ada()]).await,
        vec![person(ada(), "Ada Lovelace", "lovelace@example.com")]
    );
}

pub async fn each_member_has_their_role_in_their_project_and_nowhere_else(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    remembered(
        catalog,
        &members_of(
            "project_1",
            2,
            &[(ada(), Role::Owner), (grace(), Role::Reader)],
        ),
    )
    .await;
    remembered(
        catalog,
        &members_of("project_2", 1, &[(hedy(), Role::Owner)]),
    )
    .await;

    assert_eq!(role(catalog, "project_1", &ada()).await, Some(Role::Owner));
    assert_eq!(
        role(catalog, "project_1", &grace()).await,
        Some(Role::Reader)
    );
    assert_eq!(role(catalog, "project_1", &hedy()).await, None);
    assert_eq!(role(catalog, "project_2", &ada()).await, None);
    assert_eq!(role(catalog, "project_3", &ada()).await, None);
}

pub async fn an_account_s_memberships_are_every_project_it_is_in_by_project(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    remembered(
        catalog,
        &members_of("project_b", 1, &[(ada(), Role::Owner)]),
    )
    .await;
    remembered(
        catalog,
        &members_of("project_c", 1, &[(grace(), Role::Owner)]),
    )
    .await;
    remembered(
        catalog,
        &members_of(
            "project_a",
            2,
            &[(grace(), Role::Owner), (ada(), Role::Author)],
        ),
    )
    .await;

    assert_eq!(
        memberships(catalog, &ada()).await,
        vec![
            Membership {
                project: a_project("project_a"),
                role: Role::Author,
            },
            Membership {
                project: a_project("project_b"),
                role: Role::Owner,
            },
        ]
    );
}

pub async fn newer_members_replace_the_older_whoever_is_gone_with_them(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    remembered(
        catalog,
        &members_of(
            "project_1",
            2,
            &[(ada(), Role::Owner), (grace(), Role::Author)],
        ),
    )
    .await;

    remembered(
        catalog,
        &members_of(
            "project_1",
            4,
            &[(ada(), Role::Reader), (hedy(), Role::Owner)],
        ),
    )
    .await;

    assert_eq!(role(catalog, "project_1", &ada()).await, Some(Role::Reader));
    assert_eq!(role(catalog, "project_1", &grace()).await, None);
    assert!(memberships(catalog, &grace()).await.is_empty());
    assert_eq!(role(catalog, "project_1", &hedy()).await, Some(Role::Owner));
}

pub async fn older_members_arriving_late_change_nothing(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    remembered(
        catalog,
        &members_of(
            "project_1",
            4,
            &[(ada(), Role::Owner), (grace(), Role::Author)],
        ),
    )
    .await;

    for late in [2, 4] {
        remembered(
            catalog,
            &members_of(
                "project_1",
                late,
                &[(ada(), Role::Owner), (hedy(), Role::Reader)],
            ),
        )
        .await;
    }

    assert_eq!(
        role(catalog, "project_1", &grace()).await,
        Some(Role::Author)
    );
    assert_eq!(role(catalog, "project_1", &hedy()).await, None);
}

pub async fn what_was_remembered_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    let kept = members_of("project_1", 1, &[(ada(), Role::Owner)]);
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");
    let mut transaction = catalog.begun().await;
    let uncommitted = catalog.uncommitted();
    uncommitted
        .remember(
            &mut transaction,
            &members_of(
                "project_1",
                2,
                &[(ada(), Role::Owner), (grace(), Role::Author)],
            ),
        )
        .await
        .expect("remembering should succeed");
    uncommitted
        .remember_person(
            &mut transaction,
            &Person {
                account: hedy(),
                name: "Hedy".to_owned(),
                email: "hedy@example.com".to_owned(),
            },
        )
        .await
        .expect("remembering should succeed");

    drop(transaction);

    assert_eq!(
        catalog
            .role_of(&a_project("project_1"), &grace())
            .await
            .expect("asking should succeed"),
        None
    );
    assert!(
        catalog
            .people(&[hedy()])
            .await
            .expect("naming should succeed")
            .is_empty()
    );
}

async fn in_the_trash(catalog: &impl MembershipCatalog, project: &str) -> bool {
    catalog
        .is_in_the_trash(&ProjectLink::from(project))
        .await
        .expect("asking should succeed")
}

pub async fn a_project_is_in_the_trash_once_told_so_and_out_once_told_restored(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    assert!(
        !in_the_trash(catalog.uncommitted(), "project_1").await,
        "nothing told, nothing trashed"
    );

    catalog
        .remember_trash("project_1", 5, true)
        .await
        .expect("remembering should succeed");
    let trashed = in_the_trash(catalog.uncommitted(), "project_1").await;
    catalog
        .remember_trash("project_1", 6, false)
        .await
        .expect("remembering should succeed");

    assert!(trashed);
    assert!(!in_the_trash(catalog.uncommitted(), "project_1").await);
    assert!(
        !in_the_trash(catalog.uncommitted(), "project_2").await,
        "each project for itself"
    );
}

pub async fn an_older_trashing_heard_after_a_newer_restoring_changes_nothing(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    catalog
        .remember_trash("project_1", 6, false)
        .await
        .expect("remembering should succeed");

    catalog
        .remember_trash("project_1", 5, true)
        .await
        .expect("remembering should succeed");

    assert!(
        !in_the_trash(catalog.uncommitted(), "project_1").await,
        "a restore heard before the trashing it undoes must not leave the project trashed"
    );
}

pub async fn a_trashing_in_a_dropped_transaction_was_never_heard(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .remember_trash(
            &mut transaction,
            &ProjectLink::from("project_1"),
            Version::of(5),
            true,
        )
        .await
        .expect("remembering should succeed");

    drop(transaction);

    assert!(!in_the_trash(catalog.uncommitted(), "project_1").await);
}

pub async fn a_deleted_project_s_trash_is_forgotten_whole(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    catalog
        .remember_trash("project_1", 5, true)
        .await
        .expect("remembering should succeed");
    catalog
        .remember_trash("project_2", 5, true)
        .await
        .expect("remembering should succeed");

    catalog
        .forget_trash("project_1")
        .await
        .expect("forgetting should succeed");

    assert!(!in_the_trash(catalog.uncommitted(), "project_1").await);
    assert!(
        in_the_trash(catalog.uncommitted(), "project_2").await,
        "each project for itself"
    );
    catalog
        .remember_trash("project_1", 1, true)
        .await
        .expect("remembering should succeed");
    assert!(
        in_the_trash(catalog.uncommitted(), "project_1").await,
        "nothing of it is left, not even the version it reached"
    );
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::catalog_conformance_case!(
            $workbench,
            what_was_remembered_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::catalog_conformance_case!($workbench, an_empty_catalog_knows_nobody_in_any_project);
        $crate::catalog_conformance_case!(
            $workbench,
            each_member_has_their_role_in_their_project_and_nowhere_else
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_account_s_memberships_are_every_project_it_is_in_by_project
        );
        $crate::catalog_conformance_case!(
            $workbench,
            newer_members_replace_the_older_whoever_is_gone_with_them
        );
        $crate::catalog_conformance_case!($workbench, older_members_arriving_late_change_nothing);
        $crate::catalog_conformance_case!(
            $workbench,
            the_people_asked_for_are_told_by_account_and_no_others
        );
        $crate::catalog_conformance_case!($workbench, a_person_told_again_is_known_as_last_told);
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_is_in_the_trash_once_told_so_and_out_once_told_restored
        );
        $crate::catalog_conformance_case!($workbench, a_deleted_project_s_trash_is_forgotten_whole);
        $crate::catalog_conformance_case!(
            $workbench,
            an_older_trashing_heard_after_a_newer_restoring_changes_nothing
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_trashing_in_a_dropped_transaction_was_never_heard
        );
    };
}
