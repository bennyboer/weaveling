use std::sync::Arc;
use std::sync::atomic::Ordering;

use access::Role;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use live::testing::Subscriber;
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

const PROJECT: &str = "project_1";

struct Guarded {
    wired: Wired,
    ada: TestServer,
    grace: TestServer,
    outline: String,
    section: String,
}

fn as_account(wired: &Wired, account: &str) -> TestServer {
    TestServer::builder().http_transport().build(
        wired
            .routes
            .clone()
            .layer(Extension(Agent::Account(AccountId::from(account)))),
    )
}

async fn an_outline_ada_owns() -> Guarded {
    let wired = wiring::wired_with_accounts(Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)));
    wired.give(PROJECT, "account_ada", Some(Role::Owner)).await;
    let ada = as_account(&wired, "account_ada");
    let opened = ada
        .post("/outlines")
        .json(&json!({ "project": PROJECT }))
        .await;
    opened.assert_status_ok();
    let outline = opened.json::<Value>()["id"]
        .as_str()
        .expect("an outline has an id")
        .to_owned();
    let added = ada
        .post(&format!("/outlines/{outline}/sections"))
        .json(&json!({ "under": null, "after": null, "title": "Chapter one" }))
        .await;
    added.assert_status(StatusCode::CREATED);
    let section = added.json::<Value>()["section"]
        .as_str()
        .expect("a section has an id")
        .to_owned();
    wired.settle().await;

    Guarded {
        grace: as_account(&wired, "account_grace"),
        ada,
        wired,
        outline,
        section,
    }
}

async fn titles(guarded: &Guarded) -> Vec<Value> {
    guarded
        .ada
        .get(&format!("/outlines/{}", guarded.outline))
        .await
        .json::<Value>()["sections"]
        .as_array()
        .expect("an outline lists its sections")
        .iter()
        .map(|section| section["title"].clone())
        .collect()
}

async fn changes_by(guarded: &Guarded, account: &TestServer) -> [TestResponse; 4] {
    let (outline, section) = (&guarded.outline, &guarded.section);

    [
        account
            .post(&format!("/outlines/{outline}/sections"))
            .json(&json!({ "under": null, "after": section, "title": "Chapter two" }))
            .await,
        account
            .patch(&format!("/outlines/{outline}/sections/{section}"))
            .json(&json!({ "title": "The first chapter" }))
            .await,
        account
            .post(&format!("/outlines/{outline}/attachments"))
            .json(&json!({
                "attachment": { "kind": "idea", "id": "idea_1" },
                "section": section,
                "after": null,
            }))
            .await,
        account
            .delete(&format!("/outlines/{outline}/sections/{section}"))
            .await,
    ]
}

#[tokio::test]
async fn a_project_s_outline_is_not_there_for_whoever_may_not_read_it() {
    // given: an outline of a project grace may not read
    let guarded = an_outline_ada_owns().await;

    // when: she opens it, reads it and changes it
    let opened = guarded
        .grace
        .post("/outlines")
        .json(&json!({ "project": PROJECT }))
        .await;
    let read = guarded
        .grace
        .get(&format!("/outlines/{}", guarded.outline))
        .await;
    let [adding, retitling, attaching, removing] = changes_by(&guarded, &guarded.grace).await;

    // then: each time she is told it is not there
    for answered in [opened, read, adding, retitling, attaching, removing] {
        answered.assert_status(StatusCode::NOT_FOUND);
        assert_eq!(answered.json::<Value>()["type"], "not-found");
    }
    // when: she follows it
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("outline", "outlines", &[("outline", &guarded.outline)])
        .await;
    // then: the following ends as not there
    assert_eq!(hers.ended().await, "not-there");
    // and: the outline is unchanged
    assert_eq!(titles(&guarded).await, vec![json!("Chapter one")]);
}

#[tokio::test]
async fn a_reader_opens_reads_and_follows_the_outline_and_is_told_only_an_author_may_change_it() {
    // given: an outline of a project grace may read
    let guarded = an_outline_ada_owns().await;
    guarded
        .wired
        .give(PROJECT, "account_grace", Some(Role::Reader))
        .await;

    // when: she opens it
    let opened = guarded
        .grace
        .post("/outlines")
        .json(&json!({ "project": PROJECT }))
        .await;
    // then: she is given the outline
    opened.assert_status_ok();
    assert_eq!(opened.json::<Value>()["id"], guarded.outline.as_str());
    // and: she may read it
    guarded
        .grace
        .get(&format!("/outlines/{}", guarded.outline))
        .await
        .assert_status_ok();
    // and: she may follow it
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("outline", "outlines", &[("outline", &guarded.outline)])
        .await;
    hers.frame().await;
    // and: every change she makes is refused as not an author's
    for refused in changes_by(&guarded, &guarded.grace).await {
        refused.assert_status(StatusCode::FORBIDDEN);
        assert_eq!(refused.json::<Value>()["type"], "not-an-author");
    }
    // and: the outline is unchanged
    assert_eq!(titles(&guarded).await, vec![json!("Chapter one")]);
}

#[tokio::test]
async fn an_author_adds_retitles_attaches_and_removes() {
    // given: an outline of a project grace may write
    let guarded = an_outline_ada_owns().await;
    guarded
        .wired
        .give(PROJECT, "account_grace", Some(Role::Author))
        .await;

    // when: she adds, retitles, attaches and removes
    let [adding, retitling, attaching, removing] = changes_by(&guarded, &guarded.grace).await;

    // then: each is done
    adding.assert_status(StatusCode::CREATED);
    retitling.assert_status_ok();
    attaching.assert_status_ok();
    removing.assert_status_ok();
    // and: the outline shows it
    assert_eq!(titles(&guarded).await, vec![json!("Chapter two")]);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    // given: an outline
    let guarded = an_outline_ada_owns().await;
    // and: whom to let in cannot be known
    guarded.wired.access.down.store(true, Ordering::Release);

    // then: opening and reading it fail
    for answered in [
        guarded
            .ada
            .post("/outlines")
            .json(&json!({ "project": PROJECT }))
            .await,
        guarded
            .ada
            .get(&format!("/outlines/{}", guarded.outline))
            .await,
    ] {
        answered.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    // when: it is followed
    let mut ada_s = Subscriber::on(&guarded.ada).await;
    ada_s
        .subscribe("outline", "outlines", &[("outline", &guarded.outline)])
        .await;
    // then: the following fails
    assert_eq!(ada_s.ended().await, "failed");
}

#[tokio::test]
async fn a_follower_whose_role_is_taken_away_is_closed_out() {
    // given: an outline grace follows as a reader
    let guarded = an_outline_ada_owns().await;
    guarded
        .wired
        .give(PROJECT, "account_grace", Some(Role::Reader))
        .await;
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("outline", "outlines", &[("outline", &guarded.outline)])
        .await;
    hers.frame().await;

    // when: her role is taken away
    guarded.wired.give(PROJECT, "account_grace", None).await;

    // then: her following ends as access lost
    assert_eq!(hers.ended().await, "access-lost");
}
