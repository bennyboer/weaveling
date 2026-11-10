use std::ops::Deref;

use eventsourcing::{AccountId, Version};
use members_core::{
    CatalogError, Leaving, Member, Membership, MembershipCatalog, Person, ProjectLink,
    ProjectMembers, Role,
};
use time::OffsetDateTime;
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

    pub async fn remember_leaving(
        &self,
        account: &AccountId,
        version: u64,
        until: Option<OffsetDateTime>,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_leaving(transaction, account, Version::of(version), until)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_person(&self, account: &AccountId) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget_person(transaction, account)
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

fn a_person(account: AccountId, name: &str, email: &str) -> Person {
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
    // expect: nobody has a role
    assert_eq!(role(catalog, "project_1", &ada()).await, None);
    // and: nobody is in any project
    assert!(memberships(catalog, &ada()).await.is_empty());
    // and: nobody is known
    assert!(people(catalog, &[ada()]).await.is_empty());
}

pub async fn the_people_asked_for_are_told_by_account_and_no_others(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: three people known
    known(
        catalog,
        &a_person(hedy(), "Hedy Lamarr", "hedy@example.com"),
    )
    .await;
    known(
        catalog,
        &a_person(grace(), "Grace Hopper", "grace@example.com"),
    )
    .await;
    known(catalog, &a_person(ada(), "Ada Lovelace", "ada@example.com")).await;

    // expect: asking for two of them and someone unknown tells the two, by account
    assert_eq!(
        people(
            catalog,
            &[grace(), AccountId::from("account_nobody"), ada()]
        )
        .await,
        vec![
            a_person(ada(), "Ada Lovelace", "ada@example.com"),
            a_person(grace(), "Grace Hopper", "grace@example.com"),
        ]
    );
    // and: asking for nobody tells nobody
    assert!(people(catalog, &[]).await.is_empty());
}

pub async fn a_person_told_again_is_known_as_last_told(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: a person known
    known(catalog, &a_person(ada(), "Ada", "ada@example.com")).await;

    // when: they are told again with another name and email
    known(
        catalog,
        &a_person(ada(), "Ada Lovelace", "lovelace@example.com"),
    )
    .await;

    // then: they are known as last told
    assert_eq!(
        people(catalog, &[ada()]).await,
        vec![a_person(ada(), "Ada Lovelace", "lovelace@example.com")]
    );
}

pub async fn a_person_forgotten_is_known_no_more_and_no_other_with_them(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: two people known
    known(catalog, &a_person(ada(), "Ada Lovelace", "ada@example.com")).await;
    known(
        catalog,
        &a_person(grace(), "Grace Hopper", "grace@example.com"),
    )
    .await;

    // when: one is forgotten, twice
    for _ in 0..2 {
        catalog
            .forget_person(&ada())
            .await
            .expect("forgetting should succeed");
    }

    // then: only the other is known
    assert_eq!(
        people(catalog, &[ada(), grace()]).await,
        vec![a_person(grace(), "Grace Hopper", "grace@example.com")]
    );
}

pub async fn each_member_has_their_role_in_their_project_and_nowhere_else(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: the members of two projects remembered
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

    // expect: each member has their role in their project
    assert_eq!(role(catalog, "project_1", &ada()).await, Some(Role::Owner));
    assert_eq!(
        role(catalog, "project_1", &grace()).await,
        Some(Role::Reader)
    );
    // and: none elsewhere
    assert_eq!(role(catalog, "project_1", &hedy()).await, None);
    assert_eq!(role(catalog, "project_2", &ada()).await, None);
    assert_eq!(role(catalog, "project_3", &ada()).await, None);
}

pub async fn an_account_s_memberships_are_every_project_it_is_in_by_project(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: the members of three projects remembered, an account in two of them
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

    // expect: its memberships are those two, by project
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
    // given: a project's members remembered
    remembered(
        catalog,
        &members_of(
            "project_1",
            2,
            &[(ada(), Role::Owner), (grace(), Role::Author)],
        ),
    )
    .await;

    // when: newer members of it are remembered
    remembered(
        catalog,
        &members_of(
            "project_1",
            4,
            &[(ada(), Role::Reader), (hedy(), Role::Owner)],
        ),
    )
    .await;

    // then: whoever stayed has their new role
    assert_eq!(role(catalog, "project_1", &ada()).await, Some(Role::Reader));
    // and: whoever is gone has none and is in no project
    assert_eq!(role(catalog, "project_1", &grace()).await, None);
    assert!(memberships(catalog, &grace()).await.is_empty());
    // and: whoever came has theirs
    assert_eq!(role(catalog, "project_1", &hedy()).await, Some(Role::Owner));
}

pub async fn older_members_arriving_late_change_nothing(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: a project's members remembered
    remembered(
        catalog,
        &members_of(
            "project_1",
            4,
            &[(ada(), Role::Owner), (grace(), Role::Author)],
        ),
    )
    .await;

    // when: other members of an older or the same version are remembered
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

    // then: the members remembered first stand
    assert_eq!(
        role(catalog, "project_1", &grace()).await,
        Some(Role::Author)
    );
    assert_eq!(role(catalog, "project_1", &hedy()).await, None);
}

pub async fn what_was_remembered_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: a project's members remembered
    let kept = members_of("project_1", 1, &[(ada(), Role::Owner)]);
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");
    // and: newer members and a person remembered in a transaction
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

    // when: the transaction is dropped
    drop(transaction);

    // then: the newer member has no role
    assert_eq!(
        catalog
            .role_of(&a_project("project_1"), &grace())
            .await
            .expect("asking should succeed"),
        None
    );
    // and: the person is not known
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
    // expect: a project nothing was told of is not in the trash
    assert!(
        !in_the_trash(catalog.uncommitted(), "project_1").await,
        "nothing told, nothing trashed"
    );

    // when: it is told trashed
    catalog
        .remember_trash("project_1", 5, true)
        .await
        .expect("remembering should succeed");
    let trashed = in_the_trash(catalog.uncommitted(), "project_1").await;
    // and: then told restored
    catalog
        .remember_trash("project_1", 6, false)
        .await
        .expect("remembering should succeed");

    // then: it was in the trash in between
    assert!(trashed);
    // and: it is out of it now
    assert!(!in_the_trash(catalog.uncommitted(), "project_1").await);
    // and: no other project is in it
    assert!(
        !in_the_trash(catalog.uncommitted(), "project_2").await,
        "each project for itself"
    );
}

pub async fn an_older_trashing_heard_after_a_newer_restoring_changes_nothing(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: a project told restored
    catalog
        .remember_trash("project_1", 6, false)
        .await
        .expect("remembering should succeed");

    // when: an older trashing of it is told
    catalog
        .remember_trash("project_1", 5, true)
        .await
        .expect("remembering should succeed");

    // then: it is not in the trash
    assert!(
        !in_the_trash(catalog.uncommitted(), "project_1").await,
        "a restore heard before the trashing it undoes must not leave the project trashed"
    );
}

pub async fn a_trashing_in_a_dropped_transaction_was_never_heard(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: a project told trashed in a transaction
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

    // when: the transaction is dropped
    drop(transaction);

    // then: it is not in the trash
    assert!(!in_the_trash(catalog.uncommitted(), "project_1").await);
}

pub async fn a_deleted_project_s_trash_is_forgotten_whole(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: two projects told trashed
    catalog
        .remember_trash("project_1", 5, true)
        .await
        .expect("remembering should succeed");
    catalog
        .remember_trash("project_2", 5, true)
        .await
        .expect("remembering should succeed");

    // when: the trash of one is forgotten
    catalog
        .forget_trash("project_1")
        .await
        .expect("forgetting should succeed");

    // then: it is not in the trash
    assert!(!in_the_trash(catalog.uncommitted(), "project_1").await);
    // and: the other still is
    assert!(
        in_the_trash(catalog.uncommitted(), "project_2").await,
        "each project for itself"
    );

    // when: the forgotten one is told trashed at its first version
    catalog
        .remember_trash("project_1", 1, true)
        .await
        .expect("remembering should succeed");
    // then: it is in the trash
    assert!(
        in_the_trash(catalog.uncommitted(), "project_1").await,
        "nothing of it is left, not even the version it reached"
    );
}

fn on_the_day(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plain moment")
}

async fn leaving(catalog: &impl MembershipCatalog, accounts: &[AccountId]) -> Vec<Leaving> {
    catalog
        .leaving(accounts)
        .await
        .expect("asking should succeed")
}

pub async fn an_account_told_leaving_is_known_until_when_and_no_longer_once_kept(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // expect: nobody is leaving before anything is told
    assert!(
        leaving(catalog.uncommitted(), &[ada(), grace()])
            .await
            .is_empty()
    );

    // when: two accounts are told leaving, one of them not asked for
    catalog
        .remember_leaving(&ada(), 7, Some(on_the_day(1_800_000_000)))
        .await
        .expect("remembering should succeed");
    catalog
        .remember_leaving(&hedy(), 3, Some(on_the_day(1_800_000_500)))
        .await
        .expect("remembering should succeed");

    // then: of those asked for, the one leaving is told with its day
    assert_eq!(
        leaving(catalog.uncommitted(), &[ada(), grace()]).await,
        vec![Leaving {
            account: ada(),
            until: on_the_day(1_800_000_000),
        }]
    );
    // when: she is kept after all
    catalog
        .remember_leaving(&ada(), 8, None)
        .await
        .expect("remembering should succeed");
    // then: she is leaving no longer, the other still is
    assert!(leaving(catalog.uncommitted(), &[ada()]).await.is_empty());
    assert_eq!(leaving(catalog.uncommitted(), &[hedy()]).await.len(), 1);
}

pub async fn an_older_word_on_leaving_heard_late_changes_nothing(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: an account kept after all, at its eighth version
    catalog
        .remember_leaving(&ada(), 8, None)
        .await
        .expect("remembering should succeed");

    // when: the asking it undid, at its seventh, is heard late
    catalog
        .remember_leaving(&ada(), 7, Some(on_the_day(1_800_000_000)))
        .await
        .expect("remembering should succeed");

    // then: she is not leaving
    assert!(
        leaving(catalog.uncommitted(), &[ada()]).await.is_empty(),
        "a keeping heard before the asking it undoes must not leave her leaving"
    );
}

pub async fn leaving_told_in_a_dropped_transaction_was_never_heard(
    catalog: &Committing<'_, impl MembershipCatalog>,
) {
    // given: an account told leaving in a transaction
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .remember_leaving(
            &mut transaction,
            &ada(),
            Version::of(7),
            Some(on_the_day(1_800_000_000)),
        )
        .await
        .expect("remembering should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: she is not leaving
    assert!(leaving(catalog.uncommitted(), &[ada()]).await.is_empty());
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
            a_person_forgotten_is_known_no_more_and_no_other_with_them
        );
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
        $crate::catalog_conformance_case!(
            $workbench,
            an_account_told_leaving_is_known_until_when_and_no_longer_once_kept
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_older_word_on_leaving_heard_late_changes_nothing
        );
        $crate::catalog_conformance_case!(
            $workbench,
            leaving_told_in_a_dropped_transaction_was_never_heard
        );
    };
}
