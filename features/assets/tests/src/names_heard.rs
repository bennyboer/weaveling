use std::sync::Arc;

use accounts_contract::{SIGNED_UP, SIGNED_UP_BY_INVITATION};
use assets_core::{Held, Holders};
use assets_messaging::{RememberPeopleListener, RememberProjectNamesListener};
use clock::FixedClock;
use eventsourcing::Agent;
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::{RENAMED, STARTED};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::bench::{self, Bench, THE_LOOM};

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";
const ADA: &str = "account_ada";

fn told(routing: &str, kind: &str, id: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        kind,
        id,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn ada_signed_up(version: u64, named: &str, email: &str) -> Message {
    told(
        SIGNED_UP,
        "account",
        ADA,
        version,
        json!({
            "version": version,
            "name": "SIGNED_UP",
            "personal": { "name": named, "email": email },
        }),
    )
}

fn ada_deleted(version: u64) -> Message {
    told(
        accounts_contract::DELETED,
        "account",
        ADA,
        version,
        json!({ "version": version, "name": "DELETED" }),
    )
}

fn the_loom_named(routing: &str, version: u64, named: &str) -> Message {
    let told_as = if routing == STARTED {
        "STARTED"
    } else {
        "RENAMED"
    };
    told(
        routing,
        "project",
        THE_LOOM,
        version,
        json!({ "version": version, "name": told_as, "project_name": named }),
    )
}

fn the_loom_deleted(version: u64) -> Message {
    told(
        projects_contract::DELETED,
        "project",
        THE_LOOM,
        version,
        json!({ "version": version, "name": "DELETED" }),
    )
}

fn inboxed<H: KeptHandler>(handler: H) -> Inboxed<H> {
    Inboxed::new(
        handler,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
            1_700_000_000,
        ))))),
    )
}

async fn people_heard(bench: &Bench, messages: Vec<Message>) {
    let listener = inboxed(RememberPeopleListener::new(bench.assets.clone()));
    for message in messages {
        listener
            .handle(&message)
            .await
            .expect("a word on an account is taken in");
    }
}

async fn project_names_heard(bench: &Bench, messages: Vec<Message>) {
    let listener = inboxed(RememberProjectNamesListener::new(bench.assets.clone()));
    for message in messages {
        listener
            .handle(&message)
            .await
            .expect("a word on a project is taken in");
    }
}

/// Ada uploads a photo into the loom, so both she and the loom use some storage.
async fn ada_holding_a_photo() -> Bench {
    let bench = bench::a_bench().await;
    bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an author uploads");

    bench
}

fn names(held: &[Held]) -> Vec<(Option<&str>, Option<&str>)> {
    held.iter()
        .map(|held| (held.name.as_deref(), held.email.as_deref()))
        .collect()
}

#[tokio::test]
async fn the_listeners_hear_every_word_on_names_and_deletions() {
    // given: the listeners
    let bench = bench::a_bench().await;
    let people = inboxed(RememberPeopleListener::new(bench.assets.clone()));
    let projects = inboxed(RememberProjectNamesListener::new(bench.assets.clone()));

    // expect: each hears what names its holders and what deletes them
    for routing in [
        SIGNED_UP,
        SIGNED_UP_BY_INVITATION,
        accounts_contract::DELETED,
    ] {
        assert!(
            people.hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
    for routing in [STARTED, RENAMED, projects_contract::DELETED] {
        assert!(
            projects.hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn an_account_that_signed_up_is_named_and_found_by_its_name_or_address() {
    // given: Ada holding a photo
    let bench = ada_holding_a_photo().await;

    // when: her sign-up is heard
    people_heard(
        &bench,
        vec![ada_signed_up(1, "Ada Lovelace", "ada@example.com")],
    )
    .await;

    // then: her holding is named
    assert_eq!(
        names(&bench.held(Holders::Accounts, None).await),
        vec![(Some("Ada Lovelace"), Some("ada@example.com"))]
    );
    // and: she is found by either, whatever its case
    for term in ["LOVELACE", "ada@exa"] {
        assert_eq!(
            bench.held(Holders::Accounts, Some(term)).await.len(),
            1,
            "{term}"
        );
    }
    assert!(
        bench
            .held(Holders::Accounts, Some("grace"))
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn a_deleted_account_is_named_no_more_even_by_a_sign_up_heard_late() {
    // given: Ada holding a photo, her deletion heard before her sign-up
    let bench = ada_holding_a_photo().await;
    people_heard(
        &bench,
        vec![
            ada_deleted(5),
            ada_signed_up(1, "Ada Lovelace", "ada@example.com"),
        ],
    )
    .await;

    // then: her holding is named no more
    assert_eq!(
        names(&bench.held(Holders::Accounts, None).await),
        vec![(None, None)]
    );
    assert!(bench.held(Holders::Accounts, Some("ada")).await.is_empty());
}

#[tokio::test]
async fn a_project_is_named_by_its_newest_name_heard() {
    // given: the loom holding a photo
    let bench = ada_holding_a_photo().await;

    // when: its renaming is heard before its start
    project_names_heard(
        &bench,
        vec![
            the_loom_named(RENAMED, 2, "The Great Loom"),
            the_loom_named(STARTED, 1, "Loom"),
        ],
    )
    .await;

    // then: it is named by its newest name and found by it
    assert_eq!(
        names(&bench.held(Holders::Projects, None).await),
        vec![(Some("The Great Loom"), None)]
    );
    assert_eq!(bench.held(Holders::Projects, Some("great")).await.len(), 1);
}

#[tokio::test]
async fn a_deleted_project_is_named_no_more() {
    // given: the loom holding a photo, named
    let bench = ada_holding_a_photo().await;
    project_names_heard(&bench, vec![the_loom_named(STARTED, 1, "Loom")]).await;

    // when: its deletion is heard
    project_names_heard(&bench, vec![the_loom_deleted(4)]).await;

    // then: it is named no more
    assert_eq!(
        names(&bench.held(Holders::Projects, None).await),
        vec![(None, None)]
    );
}

#[tokio::test]
async fn the_names_of_the_deleted_are_let_go_of_with_the_tombstones() {
    // given: Ada and the loom deleted
    let bench = ada_holding_a_photo().await;
    people_heard(&bench, vec![ada_deleted(5)]).await;
    project_names_heard(&bench, vec![the_loom_deleted(4)]).await;

    // when: the tombstones laid before now are let go of
    let forgotten = bench
        .assets
        .forget_tombstones_laid_before(clock::at(1_700_000_001), 100)
        .await
        .expect("forgetting should succeed");

    // then: both names' tombstones are let go of
    assert_eq!(forgotten, 2);
    // and: what was forgotten once is not forgotten again
    assert_eq!(
        bench
            .assets
            .forget_tombstones_laid_before(clock::at(1_700_000_001), 100)
            .await
            .expect("forgetting should succeed"),
        0
    );
}

#[tokio::test]
async fn what_tells_no_name_nor_deletion_is_refused() {
    // given: the listeners
    let bench = bench::a_bench().await;

    // expect: a word that tells neither fails
    assert!(
        inboxed(RememberPeopleListener::new(bench.assets.clone()))
            .handle(&told(
                SIGNED_UP,
                "account",
                ADA,
                1,
                json!({ "version": 1, "name": "EMAIL_VERIFIED" }),
            ))
            .await
            .is_err(),
        "a message the listener cannot act on belongs in dead letters"
    );
    assert!(
        inboxed(RememberProjectNamesListener::new(bench.assets.clone()))
            .handle(&told(
                STARTED,
                "project",
                THE_LOOM,
                1,
                json!({ "version": 1, "name": "RESTORED" }),
            ))
            .await
            .is_err(),
        "a message the listener cannot act on belongs in dead letters"
    );
}
