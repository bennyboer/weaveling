use email::EmailAddress;
use eventsourcing::Version;
use invitations_core::{
    DeclineReason, InvitationCatalog, InvitationId, InvitationSummary, Outcome,
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
        issued_at: at(1_000),
        until: at(605_800),
        outcome: Outcome::Open,
        retire_after: at(605_800),
    }
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
    };
}
