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

async fn a_mailed_invitation(wired: &Wired) -> InvitationId {
    let id = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id;
    catalogued(wired, &id).await;
    wired.mailed().await;

    id
}

async fn catalogued(wired: &Wired, id: &InvitationId) {
    wired
        .invitations
        .catalogue(None, id)
        .await
        .expect("cataloguing should succeed");
}

async fn events_of(wired: &Wired, id: &InvitationId) -> Vec<(InvitationEvent, Agent)> {
    wired
        .store
        .read_from(&AggregateId::from(id), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| (recorded.event, recorded.metadata.agent))
        .collect()
}

async fn the_token(wired: &Wired) -> String {
    wired.token_mailed_to(ADA).await.as_str().to_owned()
}

#[tokio::test]
async fn whoever_holds_the_link_turns_the_invitation_down_once_in_nobody_s_name() {
    // given: a mailed invitation
    let wired = wiring::a_workbench();
    let id = a_mailed_invitation(&wired).await;
    let token = the_token(&wired).await;

    // when: it is turned down by its link twice
    for _ in 0..2 {
        wired
            .invitations
            .decline_by_token(&format!(" {token}\n"))
            .await
            .expect("the link is the proof it was sent to them");
        catalogued(&wired, &id).await;
    }

    // then: it is declined once in nobody's name
    assert!(matches!(
        events_of(&wired, &id).await.as_slice(),
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
    // given: the link of an invitation taken up
    let wired = wiring::a_workbench();
    let accepted = a_mailed_invitation(&wired).await;
    let accepted_link = the_token(&wired).await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::minutes(2));
    // and: the link of one withdrawn
    let withdrawn = a_mailed_invitation(&wired).await;
    let withdrawn_link = the_token(&wired).await;
    wired
        .invitations
        .taken_up(None, &accepted, &Agent::Account("account_ada".into()))
        .await
        .expect("an open invitation is taken up");
    wired
        .invitations
        .withdraw(&wiring::an_operator(), &withdrawn, None)
        .await
        .expect("an operator may withdraw");
    for id in [&accepted, &withdrawn] {
        catalogued(&wired, id).await;
    }

    // when: each, and a link nobody was given, is turned down
    for link in [
        accepted_link.as_str(),
        withdrawn_link.as_str(),
        "a link nobody was given",
    ] {
        let refused = wired.invitations.decline_by_token(link).await;
        // then: it is refused as invalid
        assert!(
            matches!(refused, Err(InvitationsError::InvitationInvalid)),
            "{link}: {refused:?}"
        );
    }
    // and: neither invitation changes
    assert_eq!(events_of(&wired, &accepted).await.len(), 2);
    assert_eq!(events_of(&wired, &withdrawn).await.len(), 2);
}

#[tokio::test]
async fn a_link_is_turned_down_to_its_last_moment_and_run_out_no_longer() {
    // given: an invitation at its last moment
    let on_time = wiring::a_workbench();
    let in_time = a_mailed_invitation(&on_time).await;
    on_time
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(7));
    // and: another past it
    let late = wiring::a_workbench();
    let ran_out = a_mailed_invitation(&late).await;
    late.clock
        .set(wiring::at(1_700_000_000) + Duration::days(7) + Duration::SECOND);

    // when: each is turned down by its link
    on_time
        .invitations
        .decline_by_token(&the_token(&on_time).await)
        .await
        .expect("a link still holds at its last moment");
    let refused = late
        .invitations
        .decline_by_token(&the_token(&late).await)
        .await;

    // then: the one at its last moment is declined
    assert_eq!(events_of(&on_time, &in_time).await.len(), 2);
    // and: the one past it is refused as run out and stays as it was
    assert!(
        matches!(refused, Err(InvitationsError::InvitationExpired)),
        "{refused:?}"
    );
    assert_eq!(events_of(&late, &ran_out).await.len(), 1);
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
    // given: a mailed invitation
    let wired = wiring::a_workbench();
    let id = a_mailed_invitation(&wired).await;
    let server = TestServer::new(invitations_rest::router_for_anyone(
        wired.invitations.clone(),
    ));

    // when: it is turned down by its link without signing in
    declines(&server, &the_token(&wired).await)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    // and: a link nobody was given is turned down
    let nowhere = declines(&server, "a link nobody was given").await;

    // then: the invitation is declined
    assert_eq!(events_of(&wired, &id).await.len(), 2);
    // and: the link nobody was given is refused as invalid
    nowhere.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(nowhere.json::<Value>()["type"], refused::INVITATION_INVALID);
}

#[tokio::test]
async fn a_link_run_out_is_refused_by_its_kind() {
    // given: an invitation whose link has run out
    let wired = wiring::a_workbench();
    a_mailed_invitation(&wired).await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(8));
    let server = TestServer::new(invitations_rest::router_for_anyone(
        wired.invitations.clone(),
    ));

    // when: it is turned down by its link
    let refused = declines(&server, &the_token(&wired).await).await;

    // then: it is refused as run out
    refused.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refused.json::<Value>()["type"], refused::INVITATION_EXPIRED);
}
