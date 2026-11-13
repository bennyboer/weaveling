use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::{AccessCatalog, Role};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: AccessCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    async fn cleanup(self);
}

const THE_LOOM: &str = "project_loom";
const THE_WEFT: &str = "project_weft";

fn ada() -> AccountId {
    AccountId::from("account_ada")
}

fn bo() -> AccountId {
    AccountId::from("account_bo")
}

fn grace() -> AccountId {
    AccountId::from("account_grace")
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

/// Each word told the catalog, as a feature's listener hears them.
enum Word {
    Role(&'static str, AccountId, u64, Option<Role>, i64),
    Disbanded(&'static str, u64, i64),
    Trash(&'static str, u64, bool),
    Operator(AccountId, u64, bool, i64),
}

async fn begun<W: Workbench>(bench: &W) -> Transaction {
    bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed")
}

async fn tell<W: Workbench>(transaction: &mut Transaction, bench: &W, word: Word) {
    let store = bench.store();
    match word {
        Word::Role(project, account, version, role, when) => {
            store
                .remember_role(
                    transaction,
                    project,
                    &account,
                    Version::of(version),
                    role,
                    at(when),
                )
                .await
        }
        Word::Disbanded(project, version, when) => {
            store
                .remember_disbanded(transaction, project, Version::of(version), at(when))
                .await
        }
        Word::Trash(project, version, in_the_trash) => {
            store
                .remember_trash(transaction, project, Version::of(version), in_the_trash)
                .await
        }
        Word::Operator(account, version, operator, when) => {
            store
                .remember_operator(
                    transaction,
                    &account,
                    Version::of(version),
                    operator,
                    at(when),
                )
                .await
        }
    }
    .expect("remembering should succeed");
}

async fn told<W: Workbench>(bench: &W, words: Vec<Word>) {
    let mut transaction = begun(bench).await;
    for word in words {
        tell(&mut transaction, bench, word).await;
    }
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn tombstones_forgotten<W: Workbench>(bench: &W, before: i64, at_most: u64) -> u64 {
    let mut transaction = begun(bench).await;
    let forgotten = bench
        .store()
        .forget_tombstones_laid_before(&mut transaction, at(before), at_most)
        .await
        .expect("forgetting should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    forgotten
}

async fn role<W: Workbench>(bench: &W, project: &str, account: &AccountId) -> Option<Role> {
    bench
        .store()
        .role_of(project, account)
        .await
        .expect("asking should succeed")
}

async fn projects<W: Workbench>(bench: &W, account: &AccountId) -> Vec<String> {
    let mut projects = bench
        .store()
        .projects_of(account)
        .await
        .expect("asking should succeed");
    projects.sort();

    projects
}

async fn share<W: Workbench>(bench: &W, one: &AccountId, other: &AccountId) -> bool {
    bench
        .store()
        .share_a_project(one, other)
        .await
        .expect("asking should succeed")
}

async fn trashed<W: Workbench>(bench: &W, project: &str) -> bool {
    bench
        .store()
        .is_in_the_trash(project)
        .await
        .expect("asking should succeed")
}

async fn operates<W: Workbench>(bench: &W, account: &AccountId) -> bool {
    bench
        .store()
        .operates(account)
        .await
        .expect("asking should succeed")
}

/// Ada owns the loom with Bo as its author, and Grace owns the weft.
fn the_members() -> Vec<Word> {
    vec![
        Word::Role(THE_LOOM, ada(), 1, Some(Role::Owner), 0),
        Word::Role(THE_LOOM, bo(), 2, Some(Role::Author), 0),
        Word::Role(THE_WEFT, grace(), 1, Some(Role::Owner), 0),
    ]
}

pub async fn nothing_heard_is_no_access<W: Workbench>(bench: &W) {
    // expect: no role, no project, nothing shared or in the trash, and nobody running the instance
    assert_eq!(role(bench, THE_LOOM, &ada()).await, None);
    assert!(projects(bench, &ada()).await.is_empty());
    assert!(!share(bench, &ada(), &bo()).await);
    assert!(!trashed(bench, THE_LOOM).await);
    assert!(!operates(bench, &ada()).await);
}

pub async fn a_member_has_their_role_in_their_projects_alone<W: Workbench>(bench: &W) {
    // given: the members
    told(bench, the_members()).await;

    // expect: each has their role where they are, and none elsewhere
    assert_eq!(role(bench, THE_LOOM, &ada()).await, Some(Role::Owner));
    assert_eq!(role(bench, THE_LOOM, &bo()).await, Some(Role::Author));
    assert_eq!(role(bench, THE_WEFT, &ada()).await, None);
    assert_eq!(projects(bench, &ada()).await, [THE_LOOM]);
    assert_eq!(projects(bench, &grace()).await, [THE_WEFT]);
    // and: Ada and Bo share a project, either way round, and Grace with neither
    assert!(share(bench, &ada(), &bo()).await);
    assert!(share(bench, &bo(), &ada()).await);
    assert!(!share(bench, &ada(), &grace()).await);
}

pub async fn the_newest_role_heard_holds_and_an_older_one_heard_late_changes_nothing<
    W: Workbench,
>(
    bench: &W,
) {
    // given: the members, and Bo made a reader, heard twice
    told(bench, the_members()).await;
    told(
        bench,
        vec![
            Word::Role(THE_LOOM, bo(), 4, Some(Role::Reader), 0),
            Word::Role(THE_LOOM, bo(), 4, Some(Role::Reader), 0),
        ],
    )
    .await;

    // when: his being made an owner before it is heard late
    told(
        bench,
        vec![Word::Role(THE_LOOM, bo(), 3, Some(Role::Owner), 0)],
    )
    .await;

    // then: he is a reader
    assert_eq!(role(bench, THE_LOOM, &bo()).await, Some(Role::Reader));
}

pub async fn one_who_left_has_no_role_and_a_joining_heard_late_does_not_bring_them_back<
    W: Workbench,
>(
    bench: &W,
) {
    // given: the members, and Bo left the loom
    told(bench, the_members()).await;
    told(bench, vec![Word::Role(THE_LOOM, bo(), 3, None, 1_000)]).await;
    // and: Grace's leaving of the loom heard before her joining
    told(bench, vec![Word::Role(THE_LOOM, grace(), 5, None, 1_000)]).await;
    told(
        bench,
        vec![Word::Role(THE_LOOM, grace(), 4, Some(Role::Reader), 0)],
    )
    .await;

    // expect: neither has a role in the loom, nor shares it with Ada
    assert_eq!(role(bench, THE_LOOM, &bo()).await, None);
    assert_eq!(role(bench, THE_LOOM, &grace()).await, None);
    assert!(!share(bench, &ada(), &bo()).await);
    assert!(!share(bench, &ada(), &grace()).await);
    assert!(projects(bench, &bo()).await.is_empty());
    // when: Bo joins again
    told(
        bench,
        vec![Word::Role(THE_LOOM, bo(), 6, Some(Role::Reader), 0)],
    )
    .await;
    // then: he is a reader again
    assert_eq!(role(bench, THE_LOOM, &bo()).await, Some(Role::Reader));
}

pub async fn a_project_disbanded_holds_nobody_whatever_is_heard_of_it_late<W: Workbench>(
    bench: &W,
) {
    // given: the members, and the loom disbanded
    told(bench, the_members()).await;
    told(bench, vec![Word::Disbanded(THE_LOOM, 3, 1_000)]).await;

    // when: Grace's joining of it is heard late
    told(
        bench,
        vec![Word::Role(THE_LOOM, grace(), 2, Some(Role::Reader), 0)],
    )
    .await;

    // then: nobody has a role in it or shares it, and the weft is Grace's still
    assert_eq!(role(bench, THE_LOOM, &ada()).await, None);
    assert_eq!(role(bench, THE_LOOM, &grace()).await, None);
    assert!(!share(bench, &ada(), &bo()).await);
    assert!(projects(bench, &ada()).await.is_empty());
    assert_eq!(projects(bench, &grace()).await, [THE_WEFT]);
}

pub async fn a_project_in_the_trash_keeps_its_roles_but_is_neither_listed_nor_shared<
    W: Workbench,
>(
    bench: &W,
) {
    // given: the members, and the loom put in the trash
    told(bench, the_members()).await;
    told(bench, vec![Word::Trash(THE_LOOM, 7, true)]).await;

    // expect: it is in the trash, its roles kept, but nobody's project to list or share
    assert!(trashed(bench, THE_LOOM).await);
    assert_eq!(role(bench, THE_LOOM, &ada()).await, Some(Role::Owner));
    assert!(projects(bench, &ada()).await.is_empty());
    assert!(!share(bench, &ada(), &bo()).await);
    // when: its putting in the trash is heard again, and an older restoring late
    told(
        bench,
        vec![
            Word::Trash(THE_LOOM, 7, true),
            Word::Trash(THE_LOOM, 6, false),
        ],
    )
    .await;
    // then: it is in the trash still
    assert!(trashed(bench, THE_LOOM).await);
    // when: it is restored
    told(bench, vec![Word::Trash(THE_LOOM, 8, false)]).await;
    // then: it is listed and shared again
    assert!(!trashed(bench, THE_LOOM).await);
    assert_eq!(projects(bench, &ada()).await, [THE_LOOM]);
    assert!(share(bench, &ada(), &bo()).await);
}

pub async fn the_newest_word_on_who_runs_the_instance_holds<W: Workbench>(bench: &W) {
    // given: Ada became an operator, and Bo's stopping heard before his becoming one
    told(
        bench,
        vec![
            Word::Operator(ada(), 3, true, 0),
            Word::Operator(bo(), 5, false, 1_000),
            Word::Operator(bo(), 4, true, 0),
        ],
    )
    .await;

    // expect: Ada runs the instance, and neither Bo nor Grace
    assert!(operates(bench, &ada()).await);
    assert!(!operates(bench, &bo()).await);
    assert!(!operates(bench, &grace()).await);
    // when: Ada stops
    told(bench, vec![Word::Operator(ada(), 4, false, 2_000)]).await;
    // then: she runs it no more
    assert!(!operates(bench, &ada()).await);
}

pub async fn what_ended_long_ago_is_let_go_and_what_ended_lately_still_guards<W: Workbench>(
    bench: &W,
) {
    // given: the members, Bo gone from the loom long ago, Grace's leaving of it lately
    told(bench, the_members()).await;
    told(
        bench,
        vec![
            Word::Role(THE_LOOM, bo(), 3, None, 1_000),
            Word::Role(THE_LOOM, grace(), 5, None, 5_000),
        ],
    )
    .await;
    // and: the weft disbanded long ago, and Ada stopped running the instance long ago
    told(
        bench,
        vec![
            Word::Disbanded(THE_WEFT, 2, 1_000),
            Word::Operator(ada(), 4, false, 1_000),
        ],
    )
    .await;

    // when: what ended before a moment between is let go
    let forgotten = tombstones_forgotten(bench, 3_000, 10).await;

    // then: Bo's leaving, the weft and Ada's stopping are let go, and Grace's leaving not
    assert_eq!(forgotten, 3);
    assert_eq!(tombstones_forgotten(bench, 3_000, 10).await, 0);
    // and: older words about them are taken in once nothing guards against them
    told(
        bench,
        vec![
            Word::Role(THE_LOOM, bo(), 2, Some(Role::Author), 0),
            Word::Role(THE_WEFT, grace(), 1, Some(Role::Owner), 0),
            Word::Operator(ada(), 3, true, 0),
        ],
    )
    .await;
    assert_eq!(role(bench, THE_LOOM, &bo()).await, Some(Role::Author));
    assert_eq!(role(bench, THE_WEFT, &grace()).await, Some(Role::Owner));
    assert!(operates(bench, &ada()).await);
    // and: Grace's leaving still guards against one about her
    told(
        bench,
        vec![Word::Role(THE_LOOM, grace(), 4, Some(Role::Reader), 0)],
    )
    .await;
    assert_eq!(role(bench, THE_LOOM, &grace()).await, None);
}

pub async fn no_more_tombstones_of_each_than_asked_are_let_go_at_once<W: Workbench>(bench: &W) {
    // given: three memberships ended long ago
    told(
        bench,
        vec![
            Word::Role(THE_LOOM, ada(), 2, None, 1_000),
            Word::Role(THE_LOOM, bo(), 3, None, 1_000),
            Word::Role(THE_LOOM, grace(), 4, None, 1_000),
        ],
    )
    .await;

    // expect: two are let go, then the third
    assert_eq!(tombstones_forgotten(bench, 3_000, 2).await, 2);
    assert_eq!(tombstones_forgotten(bench, 3_000, 2).await, 1);
}

pub async fn what_is_told_in_a_transaction_that_is_dropped_is_not_kept<W: Workbench>(bench: &W) {
    // given: the members and an operator told in a transaction never finished
    let mut transaction = begun(bench).await;
    for word in the_members()
        .into_iter()
        .chain([Word::Operator(ada(), 1, true, 0)])
    {
        tell(&mut transaction, bench, word).await;
    }

    // when: the transaction is dropped
    drop(transaction);

    // then: nothing is kept
    assert_eq!(role(bench, THE_LOOM, &ada()).await, None);
    assert!(!operates(bench, &ada()).await);
}

pub async fn a_project_s_owners_are_its_members_holding_it_and_nobody_once_disbanded<
    W: Workbench,
>(
    bench: &W,
) {
    // given: the members, Bo made an owner of the loom, and Grace an author in it who left
    told(bench, the_members()).await;
    told(
        bench,
        vec![
            Word::Role(THE_LOOM, bo(), 3, Some(Role::Owner), 0),
            Word::Role(THE_LOOM, grace(), 4, Some(Role::Owner), 0),
            Word::Role(THE_LOOM, grace(), 5, None, 1_000),
        ],
    )
    .await;

    // expect: the loom's owners are Ada and Bo, by their ids, and the weft's Grace
    assert_eq!(
        bench
            .store()
            .owners_of(THE_LOOM)
            .await
            .expect("asking should succeed"),
        [ada(), bo()]
    );
    assert_eq!(
        bench
            .store()
            .owners_of(THE_WEFT)
            .await
            .expect("asking should succeed"),
        [grace()]
    );
    // when: the loom is disbanded
    told(bench, vec![Word::Disbanded(THE_LOOM, 6, 2_000)]).await;
    // then: it has no owners
    assert!(
        bench
            .store()
            .owners_of(THE_LOOM)
            .await
            .expect("asking should succeed")
            .is_empty()
    );
}

#[macro_export]
macro_rules! conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::conformance_case!($workbench, nothing_heard_is_no_access);
        $crate::conformance_case!(
            $workbench,
            a_project_s_owners_are_its_members_holding_it_and_nobody_once_disbanded
        );
        $crate::conformance_case!($workbench, a_member_has_their_role_in_their_projects_alone);
        $crate::conformance_case!(
            $workbench,
            the_newest_role_heard_holds_and_an_older_one_heard_late_changes_nothing
        );
        $crate::conformance_case!(
            $workbench,
            one_who_left_has_no_role_and_a_joining_heard_late_does_not_bring_them_back
        );
        $crate::conformance_case!(
            $workbench,
            a_project_disbanded_holds_nobody_whatever_is_heard_of_it_late
        );
        $crate::conformance_case!(
            $workbench,
            a_project_in_the_trash_keeps_its_roles_but_is_neither_listed_nor_shared
        );
        $crate::conformance_case!($workbench, the_newest_word_on_who_runs_the_instance_holds);
        $crate::conformance_case!(
            $workbench,
            what_ended_long_ago_is_let_go_and_what_ended_lately_still_guards
        );
        $crate::conformance_case!(
            $workbench,
            no_more_tombstones_of_each_than_asked_are_let_go_at_once
        );
        $crate::conformance_case!(
            $workbench,
            what_is_told_in_a_transaction_that_is_dropped_is_not_kept
        );
    };
}
