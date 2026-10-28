use email::EmailAddress;
use eventsourcing::{AccountId, Version};
use invitations_core::{
    DeclineReason, InvitationCatalog, InvitationId, InvitationSummary, Outcome, Person,
    ProjectLink, Role, Scope,
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
            the_people_asked_for_are_told_by_account_and_no_others
        );
        $crate::catalog_conformance_case!($workbench, a_person_told_again_is_known_as_last_told);
    };
}
