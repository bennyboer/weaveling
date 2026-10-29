use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use invitations_contract::{DeclineInvitationRequest, refused};
use invitations_core::{DeclineReason, InvitationEvent, InvitationId, InvitationsError, KIND};
use serde_json::Value;
use time::Duration;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

async fn invited(wired: &Wired) -> InvitationId {
    let id = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id;
    catalogued(wired, &id).await;

    id
}

async fn catalogued(wired: &Wired, id: &InvitationId) {
    wired
        .invitations
        .catalogue(id)
        .await
        .expect("cataloguing should succeed");
}

async fn happened(wired: &Wired, id: &InvitationId) -> Vec<(InvitationEvent, Agent)> {
    wired
        .store
        .read_from(&AggregateId::from(id), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| (recorded.event, recorded.metadata.agent))
        .collect()
}

fn the_token(wired: &Wired) -> String {
    wired.token_mailed_to(ADA).as_str().to_owned()
}

#[tokio::test]
async fn whoever_holds_the_link_turns_the_invitation_down_once_in_nobody_s_name() {
    let wired = wiring::wired();
    let id = invited(&wired).await;
    let token = the_token(&wired);

    for _ in 0..2 {
        wired
            .invitations
            .decline_by_token(&format!(" {token}\n"))
            .await
            .expect("the link is the proof it was sent to them");
        catalogued(&wired, &id).await;
    }

    assert!(matches!(
        happened(&wired, &id).await.as_slice(),
        [
            (InvitationEvent::Issued { .. }, _),
            (
                InvitationEvent::Declined {
                    reason: DeclineReason::TurnedDown
                },
                Agent::System
            )
        ]
    ));
}

#[tokio::test]
async fn a_link_nobody_was_given_or_one_taken_up_or_withdrawn_turns_nothing_down() {
    let wired = wiring::wired();
    let accepted = invited(&wired).await;
    let accepted_link = the_token(&wired);
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::minutes(2));
    let withdrawn = invited(&wired).await;
    let withdrawn_link = the_token(&wired);
    wired
        .invitations
        .taken_up(
            &accepted,
            wiring::at(1_700_000_100),
            &Agent::Account("account_ada".into()),
        )
        .await
        .expect("an open invitation is taken up");
    wired
        .invitations
        .withdraw(&wiring::an_operator(), &withdrawn)
        .await
        .expect("an operator may withdraw");
    for id in [&accepted, &withdrawn] {
        catalogued(&wired, id).await;
    }

    for link in [
        accepted_link.as_str(),
        withdrawn_link.as_str(),
        "a link nobody was given",
    ] {
        let refused = wired.invitations.decline_by_token(link).await;
        assert!(
            matches!(refused, Err(InvitationsError::InvitationInvalid)),
            "{link}: {refused:?}"
        );
    }
    assert_eq!(happened(&wired, &accepted).await.len(), 2);
    assert_eq!(happened(&wired, &withdrawn).await.len(), 2);
}

#[tokio::test]
async fn a_link_is_turned_down_to_its_last_moment_and_run_out_no_longer() {
    let on_time = wiring::wired();
    let in_time = invited(&on_time).await;
    on_time
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(7));
    let late = wiring::wired();
    let ran_out = invited(&late).await;
    late.clock
        .set(wiring::at(1_700_000_000) + Duration::days(7) + Duration::SECOND);

    on_time
        .invitations
        .decline_by_token(&the_token(&on_time))
        .await
        .expect("a link still holds at its last moment");
    let refused = late.invitations.decline_by_token(&the_token(&late)).await;

    assert_eq!(happened(&on_time, &in_time).await.len(), 2);
    assert!(
        matches!(refused, Err(InvitationsError::InvitationExpired)),
        "{refused:?}"
    );
    assert_eq!(happened(&late, &ran_out).await.len(), 1);
}

async fn declines(server: &TestServer, token: &str) -> TestResponse {
    server
        .post("/invitations/decline")
        .json(&DeclineInvitationRequest {
            token: token.to_owned(),
        })
        .await
}

#[tokio::test]
async fn anyone_turns_an_invitation_down_by_its_link_without_signing_in() {
    let wired = wiring::wired();
    let id = invited(&wired).await;
    let server = TestServer::new(invitations_rest::router_for_anyone(
        wired.invitations.clone(),
    ));

    declines(&server, &the_token(&wired))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let nowhere = declines(&server, "a link nobody was given").await;

    assert_eq!(happened(&wired, &id).await.len(), 2);
    nowhere.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(nowhere.json::<Value>()["type"], refused::INVITATION_INVALID);
}

#[tokio::test]
async fn a_link_run_out_is_refused_by_its_kind() {
    let wired = wiring::wired();
    invited(&wired).await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(8));
    let server = TestServer::new(invitations_rest::router_for_anyone(
        wired.invitations.clone(),
    ));

    let refused = declines(&server, &the_token(&wired)).await;

    refused.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refused.json::<Value>()["type"], refused::INVITATION_EXPIRED);
}
