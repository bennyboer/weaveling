use acting::Acting;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use email::EmailAddress;
use eventsourcing::Version;
use invitations_contract::{InvitationPageDTO, InvitationSummaryDTO, ScopeDTO, refused};
use invitations_core::{
    DeclineReason, InvitationCatalog, InvitationId, InvitationSummary, Outcome, Scope,
};
use invitations_rest::PAGE;
use serde_json::Value;
use time::Duration;

use crate::wiring;
use crate::wiring::Wired;

fn served_to(wired: &Wired, acting: Acting) -> TestServer {
    TestServer::new(invitations_rest::router(wired.invitations.clone()).layer(Extension(acting)))
}

fn a_summary(seconds: i64, to: &str, outcome: Outcome) -> InvitationSummary {
    InvitationSummary {
        id: InvitationId::generate(wiring::at(seconds)),
        version: Version::of(1),
        to: EmailAddress::new(to).expect("a plain address"),
        issued_by: None,
        issued_at: wiring::at(seconds),
        until: wiring::at(seconds) + Duration::days(7),
        outcome,
        retire_after: wiring::at(seconds) + Duration::days(7),
        scope: Scope::Instance,
    }
}

async fn remembered(wired: &Wired, summary: &InvitationSummary) {
    wired
        .catalog
        .remember(summary)
        .await
        .expect("remembering should succeed");
}

fn refused_as(response: &TestResponse, status: StatusCode, kind: &str) {
    response.assert_status(status);
    assert_eq!(response.json::<Value>()["type"], kind);
}

#[tokio::test]
async fn an_operator_sees_the_open_the_expired_and_the_declined_newest_first() {
    let wired = wiring::wired();
    let now = 1_700_000_000;
    let expired = a_summary(now - 8 * 86_400, "old@example.com", Outcome::Open);
    let declined = a_summary(
        now - 3_600,
        "ada@example.com",
        Outcome::Declined(DeclineReason::AddressHasAccount),
    );
    let open = a_summary(now - 60, "grace@example.com", Outcome::Open);
    for summary in [&expired, &declined, &open] {
        remembered(&wired, summary).await;
    }
    remembered(
        &wired,
        &a_summary(now - 120, "hedy@example.com", Outcome::Accepted),
    )
    .await;

    let page: InvitationPageDTO = served_to(&wired, wiring::an_operator())
        .get("/invitations")
        .await
        .json();

    assert_eq!(
        page,
        InvitationPageDTO {
            invitations: vec![
                InvitationSummaryDTO {
                    id: open.id.to_string(),
                    email: "grace@example.com".to_owned(),
                    issued_at: "2023-11-14T22:12:20Z".to_owned(),
                    until: "2023-11-21T22:12:20Z".to_owned(),
                    status: "open".to_owned(),
                    declined_for: None,
                    scope: ScopeDTO::Instance,
                },
                InvitationSummaryDTO {
                    id: declined.id.to_string(),
                    email: "ada@example.com".to_owned(),
                    issued_at: "2023-11-14T21:13:20Z".to_owned(),
                    until: "2023-11-21T21:13:20Z".to_owned(),
                    status: "declined".to_owned(),
                    declined_for: Some("address-has-account".to_owned()),
                    scope: ScopeDTO::Instance,
                },
                InvitationSummaryDTO {
                    id: expired.id.to_string(),
                    email: "old@example.com".to_owned(),
                    issued_at: "2023-11-06T22:13:20Z".to_owned(),
                    until: "2023-11-13T22:13:20Z".to_owned(),
                    status: "expired".to_owned(),
                    declined_for: None,
                    scope: ScopeDTO::Instance,
                },
            ],
            next: None,
        }
    );
}

#[tokio::test]
async fn a_long_list_comes_in_pages_and_the_next_goes_on_after_the_last() {
    let wired = wiring::wired();
    let pages = i64::try_from(PAGE).expect("a page fits");
    for n in 0..=pages {
        remembered(
            &wired,
            &a_summary(
                1_699_000_000 + n,
                &format!("someone{n}@example.com"),
                Outcome::Open,
            ),
        )
        .await;
    }
    let server = served_to(&wired, wiring::an_operator());

    let first: InvitationPageDTO = server.get("/invitations").await.json();
    let next: InvitationPageDTO = server
        .get("/invitations")
        .add_query_param("after", first.next.as_deref().expect("there is more"))
        .await
        .json();

    assert_eq!(first.invitations.len(), PAGE);
    assert_eq!(next.invitations.len(), 1);
    assert_eq!(next.invitations[0].email, "someone0@example.com");
    assert_eq!(next.next, None);
}

#[tokio::test]
async fn only_an_operator_sees_the_list_and_no_page_begins_after_nonsense() {
    let wired = wiring::wired();

    refused_as(
        &served_to(&wired, wiring::no_operator())
            .get("/invitations")
            .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    refused_as(
        &served_to(&wired, wiring::an_operator())
            .get("/invitations")
            .add_query_param("after", "not-an-id")
            .await,
        StatusCode::BAD_REQUEST,
        refused::INVITATION_ID_INVALID,
    );
}

#[tokio::test]
async fn a_list_that_fills_one_page_exactly_promises_no_next() {
    let wired = wiring::wired();
    let pages = i64::try_from(PAGE).expect("a page fits");
    for n in 0..pages {
        remembered(
            &wired,
            &a_summary(
                1_699_000_000 + n,
                &format!("someone{n}@example.com"),
                Outcome::Open,
            ),
        )
        .await;
    }

    let page: InvitationPageDTO = served_to(&wired, wiring::an_operator())
        .get("/invitations")
        .await
        .json();

    assert_eq!(page.invitations.len(), PAGE);
    assert_eq!(page.next, None);
}
