use email::EmailAddress;
use eventsourcing::{AccountId, Version};
use invitations_core::{
    DeclineReason, InvitationCatalog, InvitationId, InvitationSummary, Outcome, Person,
    ProjectLink, ProjectName, Role, Scope,
};
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: InvitationCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub fn a_summary(id: InvitationId, to: &str) -> InvitationSummary {
    InvitationSummary {
        id,
        version: Version::of(1),
        to: EmailAddress::new(to).expect("a plain address"),
        scope: Scope::Instance,
        issued_by: Some(AccountId::from("account_grace")),
        issued_at: at(1_000),
        until: at(605_800),
        outcome: Outcome::Open,
        retire_after: at(605_800),
    }
}

fn into(project: &str, role: Role, id: InvitationId) -> InvitationSummary {
    InvitationSummary {
        scope: Scope::Project {
            project: ProjectLink::from(project),
            role,
        },
        ..a_summary(id, "grace@example.com")
    }
}

async fn in_project(
    catalog: &impl InvitationCatalog,
    project: &str,
    after: Option<&InvitationId>,
    at_most: usize,
) -> Vec<InvitationSummary> {
    catalog
        .in_project_after(&ProjectLink::from(project), after, at_most)
        .await
        .expect("listing should succeed")
}

pub async fn an_invitation_into_a_project_is_listed_with_that_project_alone(
    catalog: &impl InvitationCatalog,
) {
    let to_the_instance = a_summary(issued(1_000), "ada@example.com");
    let into_the_loom = into("project_loom", Role::Reader, issued(2_000));
    let into_the_weft = into("project_weft", Role::Owner, issued(3_000));
    for summary in [&to_the_instance, &into_the_loom, &into_the_weft] {
        remembered(catalog, summary).await;
    }

    assert_eq!(
        listed(catalog).await,
        vec![to_the_instance],
        "the operators' list holds the instance's invitations, not those into projects"
    );
    assert_eq!(
        in_project(catalog, "project_loom", None, 100).await,
        vec![into_the_loom]
    );
    assert_eq!(
        in_project(catalog, "project_weft", None, 100).await,
        vec![into_the_weft]
    );
    assert!(
        in_project(catalog, "project_none", None, 100)
            .await
            .is_empty()
    );
}

pub async fn a_project_s_page_holds_at_most_what_was_asked_newest_first_and_goes_on_after_it(
    catalog: &impl InvitationCatalog,
) {
    let ids: Vec<InvitationId> = (1..=3).map(|nth| issued(nth * 1_000)).collect();
    for (id, role) in ids.iter().zip([Role::Owner, Role::Author, Role::Reader]) {
        remembered(catalog, &into("project_loom", role, *id)).await;
    }
    let mut taken_up = into("project_loom", Role::Author, issued(4_000));
    taken_up.outcome = Outcome::Accepted;
    remembered(catalog, &taken_up).await;

    let first = in_project(catalog, "project_loom", None, 2).await;
    let rest = in_project(catalog, "project_loom", Some(&first[1].id), 2).await;

    assert_eq!(
        first.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        vec![ids[2], ids[1]],
        "newest first, and one taken up no longer listed"
    );
    assert_eq!(
        rest.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        vec![ids[0]]
    );
    assert_eq!(
        first[0].scope,
        Scope::Project {
            project: ProjectLink::from("project_loom"),
            role: Role::Reader
        },
        "the role it invites as reads back"
    );
}

fn issued(seconds: i64) -> InvitationId {
    InvitationId::generate(at(seconds))
}

fn declined() -> Outcome {
    Outcome::Declined(DeclineReason::AddressHasAccount)
}

async fn listed(catalog: &impl InvitationCatalog) -> Vec<InvitationSummary> {
    catalog
        .listed_after(None, 100)
        .await
        .expect("listing should succeed")
}

async fn remembered(catalog: &impl InvitationCatalog, summary: &InvitationSummary) {
    catalog
        .remember(summary)
        .await
        .expect("remembering should succeed");
}

pub async fn a_remembered_invitation_is_listed_as_it_was(catalog: &impl InvitationCatalog) {
    let summary = InvitationSummary {
        version: Version::of(3),
        issued_at: at(1_700_000_000),
        until: at(1_700_604_800),
        ..a_summary(issued(1_000), "ada@example.com")
    };

    remembered(catalog, &summary).await;

    assert_eq!(listed(catalog).await, vec![summary]);
}

pub async fn an_empty_list_lists_nothing(catalog: &impl InvitationCatalog) {
    assert!(listed(catalog).await.is_empty());
}

pub async fn a_newer_summary_replaces_the_older(catalog: &impl InvitationCatalog) {
    let id = issued(1_000);
    remembered(catalog, &a_summary(id, "ada@example.com")).await;
    let newer = InvitationSummary {
        version: Version::of(2),
        outcome: declined(),
        ..a_summary(id, "ada@example.com")
    };

    remembered(catalog, &newer).await;

    assert_eq!(
        listed(catalog).await,
        vec![newer],
        "a declined invitation stays listed and says why"
    );
}

pub async fn an_older_summary_arriving_late_changes_nothing(catalog: &impl InvitationCatalog) {
    let id = issued(1_000);
    let accepted = InvitationSummary {
        version: Version::of(2),
        outcome: Outcome::Accepted,
        ..a_summary(id, "ada@example.com")
    };
    remembered(catalog, &accepted).await;

    remembered(catalog, &a_summary(id, "ada@example.com")).await;

    assert!(
        listed(catalog).await.is_empty(),
        "an invitation taken up must not come back open because its issue was heard late"
    );
}

pub async fn an_invitation_taken_up_or_withdrawn_leaves_the_list(catalog: &impl InvitationCatalog) {
    let open = a_summary(issued(1_000), "ada@example.com");
    let declined = InvitationSummary {
        outcome: declined(),
        ..a_summary(issued(2_000), "bob@example.com")
    };
    for summary in [
        open.clone(),
        declined.clone(),
        InvitationSummary {
            outcome: Outcome::Accepted,
            ..a_summary(issued(3_000), "grace@example.com")
        },
        InvitationSummary {
            outcome: Outcome::Withdrawn,
            ..a_summary(issued(4_000), "hedy@example.com")
        },
    ] {
        remembered(catalog, &summary).await;
    }

    assert_eq!(listed(catalog).await, vec![declined, open]);
}

pub async fn a_page_holds_at_most_what_was_asked_newest_first_and_the_next_goes_on_after_it(
    catalog: &impl InvitationCatalog,
) {
    for seconds in [1_000, 2_000, 3_000] {
        remembered(
            catalog,
            &a_summary(issued(seconds), &format!("someone{seconds}@example.com")),
        )
        .await;
    }
    remembered(
        catalog,
        &InvitationSummary {
            outcome: Outcome::Withdrawn,
            ..a_summary(issued(2_500), "withdrawn@example.com")
        },
    )
    .await;

    let first = catalog
        .listed_after(None, 2)
        .await
        .expect("listing should succeed");
    let next = catalog
        .listed_after(Some(&first[1].id), 2)
        .await
        .expect("listing should succeed");

    let addresses = |page: &[InvitationSummary]| -> Vec<String> {
        page.iter().map(|summary| summary.to.to_string()).collect()
    };
    assert_eq!(
        addresses(&first),
        ["someone3000@example.com", "someone2000@example.com"],
        "what has left the list takes no room on a page"
    );
    assert_eq!(addresses(&next), ["someone1000@example.com"]);
}

pub async fn what_is_past_its_moment_is_due_for_retiring_soonest_first_and_no_more_than_asked(
    catalog: &impl InvitationCatalog,
) {
    let soonest = InvitationSummary {
        retire_after: at(1_000),
        ..a_summary(issued(3_000), "soonest@example.com")
    };
    let next = InvitationSummary {
        retire_after: at(1_500),
        ..a_summary(issued(1_000), "next@example.com")
    };
    let third = InvitationSummary {
        retire_after: at(1_800),
        ..a_summary(issued(2_000), "third@example.com")
    };
    let at_its_moment = InvitationSummary {
        retire_after: at(2_000),
        ..a_summary(issued(4_000), "now@example.com")
    };
    for summary in [&third, &soonest, &at_its_moment, &next] {
        remembered(catalog, summary).await;
    }

    assert_eq!(
        catalog
            .due_for_retiring(at(2_000), 2)
            .await
            .expect("asking should succeed"),
        vec![soonest.id, next.id]
    );
    assert_eq!(
        catalog
            .due_for_retiring(at(2_000), 10)
            .await
            .expect("asking should succeed"),
        vec![soonest.id, next.id, third.id],
        "one at its very moment is not due yet"
    );
}

pub async fn a_summary_heard_anew_is_due_when_its_newest_says(catalog: &impl InvitationCatalog) {
    let id = issued(1_000);
    remembered(
        catalog,
        &InvitationSummary {
            retire_after: at(1_500),
            ..a_summary(id, "ada@example.com")
        },
    )
    .await;
    remembered(
        catalog,
        &InvitationSummary {
            version: Version::of(2),
            outcome: declined(),
            retire_after: at(5_000),
            ..a_summary(id, "ada@example.com")
        },
    )
    .await;

    assert!(
        catalog
            .due_for_retiring(at(2_000), 10)
            .await
            .expect("asking should succeed")
            .is_empty(),
        "declined later, it is kept longer"
    );
}

pub async fn a_forgotten_invitation_leaves_the_list_and_the_others_stay(
    catalog: &impl InvitationCatalog,
) {
    let gone = a_summary(issued(1_000), "gone@example.com");
    let stays = a_summary(issued(2_000), "stays@example.com");
    remembered(catalog, &gone).await;
    remembered(catalog, &stays).await;

    catalog
        .forget(&gone.id)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget(&issued(9_000))
        .await
        .expect("forgetting what was never listed is harmless");

    assert_eq!(listed(catalog).await, vec![stays]);
}

fn person(account: &str, name: &str, email: &str) -> Person {
    Person {
        account: AccountId::from(account),
        name: name.to_owned(),
        email: EmailAddress::new(email).expect("a plain address"),
    }
}

async fn known(catalog: &impl InvitationCatalog, person: &Person) {
    catalog
        .remember_person(person)
        .await
        .expect("remembering a person should succeed");
}

async fn people(catalog: &impl InvitationCatalog, accounts: &[&str]) -> Vec<Person> {
    let accounts: Vec<AccountId> = accounts.iter().copied().map(AccountId::from).collect();

    catalog
        .people(&accounts)
        .await
        .expect("asking for people should succeed")
}

pub async fn the_people_asked_for_are_told_by_account_and_no_others(
    catalog: &impl InvitationCatalog,
) {
    known(
        catalog,
        &person("account_hedy", "Hedy Lamarr", "hedy@example.com"),
    )
    .await;
    known(
        catalog,
        &person("account_grace", "Grace Hopper", "grace@example.com"),
    )
    .await;
    known(
        catalog,
        &person("account_ada", "Ada Lovelace", "ada@example.com"),
    )
    .await;

    assert!(people(catalog, &[]).await.is_empty());
    assert_eq!(
        people(catalog, &["account_grace", "account_nobody", "account_ada"]).await,
        vec![
            person("account_ada", "Ada Lovelace", "ada@example.com"),
            person("account_grace", "Grace Hopper", "grace@example.com"),
        ]
    );
}

pub async fn a_person_told_again_is_known_as_last_told(catalog: &impl InvitationCatalog) {
    known(catalog, &person("account_ada", "Ada", "ada@example.com")).await;

    known(
        catalog,
        &person("account_ada", "Ada Lovelace", "lovelace@example.com"),
    )
    .await;

    assert_eq!(
        people(catalog, &["account_ada"]).await,
        vec![person(
            "account_ada",
            "Ada Lovelace",
            "lovelace@example.com"
        )]
    );
}

fn named(project: &str, version: u64, name: &str) -> ProjectName {
    ProjectName {
        project: ProjectLink::from(project),
        version: Version::of(version),
        name: name.to_owned(),
    }
}

async fn name_known(catalog: &impl InvitationCatalog, named: &ProjectName) {
    catalog
        .remember_project_name(named)
        .await
        .expect("remembering a project's name should succeed");
}

async fn names(catalog: &impl InvitationCatalog, projects: &[&str]) -> Vec<ProjectName> {
    let projects: Vec<ProjectLink> = projects.iter().copied().map(ProjectLink::from).collect();

    catalog
        .project_names(&projects)
        .await
        .expect("asking for names should succeed")
}

pub async fn the_project_names_asked_for_are_told_by_project_and_no_others(
    catalog: &impl InvitationCatalog,
) {
    name_known(catalog, &named("project_c", 1, "The Weft")).await;
    name_known(catalog, &named("project_b", 1, "The Shuttle")).await;
    name_known(catalog, &named("project_a", 2, "The Loom")).await;

    assert!(names(catalog, &[]).await.is_empty());
    assert_eq!(
        names(catalog, &["project_b", "project_nowhere", "project_a"]).await,
        vec![
            named("project_a", 2, "The Loom"),
            named("project_b", 1, "The Shuttle"),
        ]
    );
}

pub async fn a_newer_name_replaces_the_older_and_an_older_one_heard_late_changes_nothing(
    catalog: &impl InvitationCatalog,
) {
    name_known(catalog, &named("project_a", 2, "The Loom")).await;

    name_known(catalog, &named("project_a", 4, "The Silent Loom")).await;
    for late in [1, 4] {
        name_known(catalog, &named("project_a", late, "Untitled")).await;
    }

    assert_eq!(
        names(catalog, &["project_a"]).await,
        vec![named("project_a", 4, "The Silent Loom")]
    );
}

async fn tombstoned(catalog: &impl InvitationCatalog, project: &str, laid: i64) {
    catalog
        .lay_project_tombstone(&ProjectLink::from(project), at(laid))
        .await
        .expect("laying a tombstone should succeed");
}

async fn forget_long_gone(catalog: &impl InvitationCatalog, before: i64, at_most: u64) -> u64 {
    catalog
        .forget_project_tombstones_laid_before(at(before), at_most)
        .await
        .expect("forgetting tombstones should succeed")
}

pub async fn a_deleted_project_s_name_is_told_no_more_and_a_name_heard_late_brings_it_not_back(
    catalog: &impl InvitationCatalog,
) {
    name_known(catalog, &named("project_a", 2, "The Loom")).await;
    name_known(catalog, &named("project_b", 1, "The Shuttle")).await;

    tombstoned(catalog, "project_a", 1_000).await;
    tombstoned(catalog, "project_c", 1_000).await;
    name_known(catalog, &named("project_a", 3, "The Silent Loom")).await;
    name_known(catalog, &named("project_c", 1, "The Weft")).await;

    assert_eq!(
        names(catalog, &["project_a", "project_b", "project_c"]).await,
        vec![named("project_b", 1, "The Shuttle")]
    );
}

async fn has_tombstone(catalog: &impl InvitationCatalog, project: &str) -> bool {
    catalog
        .has_project_tombstone(&ProjectLink::from(project))
        .await
        .expect("asking for a tombstone should succeed")
}

pub async fn a_project_has_a_tombstone_from_its_deletion_until_it_is_forgotten(
    catalog: &impl InvitationCatalog,
) {
    name_known(catalog, &named("project_a", 1, "The Loom")).await;
    assert!(!has_tombstone(catalog, "project_a").await);

    tombstoned(catalog, "project_a", 1_000).await;
    assert!(has_tombstone(catalog, "project_a").await);
    assert!(!has_tombstone(catalog, "project_b").await);

    forget_long_gone(catalog, 2_000, 10).await;
    assert!(!has_tombstone(catalog, "project_a").await);
}

pub async fn a_project_s_tombstone_is_laid_once_and_forgotten_oldest_first_and_at_most_so_many(
    catalog: &impl InvitationCatalog,
) {
    tombstoned(catalog, "project_newest", 3_000).await;
    tombstoned(catalog, "project_older", 2_000).await;
    tombstoned(catalog, "project_oldest", 1_000).await;
    tombstoned(catalog, "project_oldest", 2_900).await;

    assert_eq!(forget_long_gone(catalog, 2_500, 1).await, 1);
    name_known(catalog, &named("project_oldest", 1, "Forgotten")).await;
    name_known(catalog, &named("project_older", 1, "Still gone")).await;
    assert_eq!(
        names(catalog, &["project_oldest", "project_older"]).await,
        vec![named("project_oldest", 1, "Forgotten")],
        "the oldest went first, as when it was first laid; once forgotten, nothing late can come"
    );
    assert_eq!(forget_long_gone(catalog, 2_500, 5).await, 1);
    assert_eq!(
        forget_long_gone(catalog, 2_500, 5).await,
        0,
        "the newest is not old enough"
    );
}

async fn open_to(catalog: &impl InvitationCatalog, to: &str) -> Vec<InvitationId> {
    catalog
        .open_to(&EmailAddress::new(to).expect("a plain address"))
        .await
        .expect("asking for an address's invitations should succeed")
        .into_iter()
        .map(|summary| summary.id)
        .collect()
}

pub async fn the_open_invitations_to_an_address_are_told_newest_first_and_no_others(
    catalog: &impl InvitationCatalog,
) {
    let older = InvitationId::generate(at(1_000));
    let newer = InvitationId::generate(at(2_000));
    let elsewhere = InvitationId::generate(at(3_000));
    remembered(catalog, &into("project_b", Role::Reader, older)).await;
    remembered(catalog, &into("project_a", Role::Author, newer)).await;
    remembered(catalog, &a_summary(elsewhere, "hedy@example.com")).await;
    for (seconds, outcome) in [
        (4_000, Outcome::Accepted),
        (5_000, Outcome::Withdrawn),
        (6_000, Outcome::Declined(DeclineReason::AddressHasAccount)),
    ] {
        remembered(
            catalog,
            &InvitationSummary {
                outcome,
                ..into(
                    "project_a",
                    Role::Author,
                    InvitationId::generate(at(seconds)),
                )
            },
        )
        .await;
    }

    assert_eq!(
        open_to(catalog, " Grace@Example.com ").await,
        vec![newer, older]
    );
    assert_eq!(open_to(catalog, "hedy@example.com").await, vec![elsewhere]);
    assert!(open_to(catalog, "nobody@example.com").await.is_empty());
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::catalog_conformance_case!($workbench, a_remembered_invitation_is_listed_as_it_was);
        $crate::catalog_conformance_case!($workbench, an_empty_list_lists_nothing);
        $crate::catalog_conformance_case!(
            $workbench,
            what_is_past_its_moment_is_due_for_retiring_soonest_first_and_no_more_than_asked
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_invitation_leaves_the_list_and_the_others_stay
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_summary_heard_anew_is_due_when_its_newest_says
        );
        $crate::catalog_conformance_case!($workbench, a_newer_summary_replaces_the_older);
        $crate::catalog_conformance_case!(
            $workbench,
            an_older_summary_arriving_late_changes_nothing
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_invitation_taken_up_or_withdrawn_leaves_the_list
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_page_holds_at_most_what_was_asked_newest_first_and_the_next_goes_on_after_it
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_invitation_into_a_project_is_listed_with_that_project_alone
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_s_page_holds_at_most_what_was_asked_newest_first_and_goes_on_after_it
        );
        $crate::catalog_conformance_case!(
            $workbench,
            the_open_invitations_to_an_address_are_told_newest_first_and_no_others
        );
        $crate::catalog_conformance_case!(
            $workbench,
            the_people_asked_for_are_told_by_account_and_no_others
        );
        $crate::catalog_conformance_case!($workbench, a_person_told_again_is_known_as_last_told);
        $crate::catalog_conformance_case!(
            $workbench,
            the_project_names_asked_for_are_told_by_project_and_no_others
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_newer_name_replaces_the_older_and_an_older_one_heard_late_changes_nothing
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_deleted_project_s_name_is_told_no_more_and_a_name_heard_late_brings_it_not_back
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_s_tombstone_is_laid_once_and_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_has_a_tombstone_from_its_deletion_until_it_is_forgotten
        );
    };
}
