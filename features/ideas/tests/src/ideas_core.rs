use std::sync::Arc;

use crate::wiring;
use crate::wiring::Wired;
use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventStore, FailsAppending, InMemoryEventStore, ServiceError,
    StoreError, Version,
};
use ideas_catalog::InMemoryIdeaCatalog;
use time::{Duration, OffsetDateTime};
use transacting::{InMemoryTransactions, Transactions};

use ideas_core::{
    IdeaError, IdeaEvent, IdeaId, IdeaTitle, IdeasCore, IdeasError, KIND, ProjectLink,
};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_workbench() -> (IdeasCore, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::new(at(1_000)));

    (wiring::wired(clock.clone()).ideas, clock)
}

fn an_author() -> Agent {
    Agent::Account(AccountId::from("author-7"))
}

async fn a_captured_idea(ideas: &IdeasCore) -> IdeaId {
    ideas
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed")
        .0
}

#[tokio::test]
async fn capturing_yields_an_id_the_idea_can_be_fetched_with() {
    // given: a workbench
    let (ideas, _) = a_workbench();

    // when: an idea is captured
    let id = a_captured_idea(&ideas).await;
    // and: fetched by the id it was given
    let idea = ideas
        .read(&id.to_string(), &an_author())
        .await
        .expect("the idea exists");

    // then: it is the idea that was captured
    assert_eq!(idea.state.title().as_str(), "The Loom");
    assert_eq!(idea.state.project(), &ProjectLink::from("project_1"));
    assert_eq!(idea.version, Version::of(1));
}

#[tokio::test]
async fn the_client_never_computes_an_idea_id() {
    // given: a workbench
    let (ideas, _) = a_workbench();

    // when: an idea is captured
    let id = a_captured_idea(&ideas).await;

    // then: its id says it is an idea
    assert!(
        id.to_string().starts_with("idea_"),
        "an id should say what it is: {id}"
    );
}

#[tokio::test]
async fn two_ideas_captured_in_the_same_instant_still_differ() {
    // given: a workbench whose clock stands still
    let (ideas, _) = a_workbench();

    // when: two ideas are captured
    let one = a_captured_idea(&ideas).await;
    let other = a_captured_idea(&ideas).await;

    // then: their ids differ
    assert_ne!(one, other, "a fixed clock must not compact two ideas");
}

#[tokio::test]
async fn an_idea_may_be_captured_with_no_title() {
    // given: a workbench
    let (ideas, _) = a_workbench();

    // when: an idea is captured with no title
    let id = ideas
        .capture("project_1", "", &an_author())
        .await
        .expect("an idea arrives before its name")
        .0;

    // then: it stands untitled
    assert!(
        ideas
            .read(&id.to_string(), &an_author())
            .await
            .expect("it exists")
            .state
            .title()
            .is_untitled()
    );
}

#[tokio::test]
async fn a_title_the_domain_refuses_never_reaches_the_store() {
    // given: a workbench
    let (ideas, _) = a_workbench();

    // when: an idea is captured with a title past its limit
    let refused = ideas
        .capture(
            "project_1",
            &"a".repeat(IdeaTitle::MAX_CHARS + 1),
            &an_author(),
        )
        .await
        .expect_err("a sprawling title is not a title");

    // then: the title is refused
    assert!(matches!(refused, IdeasError::InvalidTitle(_)));
}

#[tokio::test]
async fn retitling_changes_what_the_idea_is_called() {
    // given: a captured idea
    let (ideas, _) = a_workbench();
    let id = a_captured_idea(&ideas).await;

    // when: it is retitled
    let landed = ideas
        .retitle(&id.to_string(), "The Silent Loom", None, &an_author())
        .await
        .expect("retitling should succeed")
        .version;

    // then: it moves on a version
    assert_eq!(landed, Version::of(2));
    // and: it is called by its new title
    assert_eq!(
        ideas
            .read(&id.to_string(), &an_author())
            .await
            .expect("it exists")
            .state
            .title()
            .as_str(),
        "The Silent Loom"
    );
}

#[tokio::test]
async fn retitling_to_the_same_title_is_accepted_and_records_nothing() {
    // given: a captured idea
    let (ideas, _) = a_workbench();
    let id = a_captured_idea(&ideas).await;

    // when: it is retitled to the title it has
    let landed = ideas
        .retitle(&id.to_string(), "The Loom", None, &an_author())
        .await
        .expect("an unchanged title is not an error")
        .version;

    // then: its version stays
    assert_eq!(
        landed,
        Version::of(1),
        "the version must not move when nothing happened"
    );
}

#[tokio::test]
async fn a_discarded_idea_refuses_further_changes() {
    // given: a discarded idea
    let (ideas, _) = a_workbench();
    let id = a_captured_idea(&ideas).await;
    ideas
        .discard(&id.to_string(), None, &an_author())
        .await
        .expect("discarding should succeed");

    // when: it is retitled
    let refused = ideas
        .retitle(&id.to_string(), "Too late", None, &an_author())
        .await
        .expect_err("a discarded idea accepts nothing");

    // then: the retitling is refused
    assert!(matches!(
        refused,
        IdeasError::Events(ServiceError::Refused(IdeaError::Discarded))
    ));
}

#[tokio::test]
async fn fetching_an_idea_that_was_never_captured_is_not_found() {
    // given: an id never captured
    let (ideas, _) = a_workbench();
    let never = IdeaId::generate(at(2_000));

    // when: it is fetched
    let missing = ideas
        .read(&never.to_string(), &an_author())
        .await
        .expect_err("nothing is there");

    // then: it is not found
    assert!(matches!(
        missing,
        IdeasError::Events(ServiceError::NotFound { .. })
    ));
}

#[tokio::test]
async fn an_id_of_another_kind_is_refused_before_the_store_is_touched() {
    // given: an id of another kind
    let (ideas, _) = a_workbench();
    let theirs = format!("scene_{}", IdeaId::generate(at(1_000)).as_uuid());

    // then: fetching by it is refused
    assert!(matches!(
        ideas
            .read(&theirs, &an_author())
            .await
            .expect_err("wrong kind of id"),
        IdeasError::InvalidId(_)
    ));
    // and: retitling by it is refused
    assert!(matches!(
        ideas
            .retitle(&theirs, "Nowhere", None, &an_author())
            .await
            .expect_err("wrong kind of id"),
        IdeasError::InvalidId(_)
    ));
}

#[tokio::test]
async fn an_idea_captured_later_sorts_after_one_captured_earlier() {
    // given: a workbench
    let (ideas, clock) = a_workbench();

    // when: an idea is captured
    let earliest = a_captured_idea(&ideas).await;
    // and: another one later
    clock.set(at(2_000));
    let latest = a_captured_idea(&ideas).await;

    // then: the earlier sorts first
    assert!(
        earliest < latest,
        "ids should sort by when the idea arrived"
    );
}

#[tokio::test]
async fn an_idea_is_snapshotted_once_the_threshold_is_reached() {
    // given: an idea with ninety-nine events
    let Wired { ideas, store, .. } = wiring::wired(Arc::new(FixedClock::new(at(1_000))));
    let id = ideas
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed")
        .0;
    ideas
        .retitle(&id.to_string(), "Title 2", None, &an_author())
        .await
        .expect("retitling should succeed");

    for counted in 3..100 {
        ideas
            .retitle(
                &id.to_string(),
                &format!("Title {counted}"),
                None,
                &an_author(),
            )
            .await
            .expect("retitling should succeed");
    }

    // then: it has no snapshot yet
    assert!(
        store
            .latest_snapshot(&AggregateId::from(&id), KIND)
            .await
            .expect("looking should succeed")
            .is_none(),
        "ninety-nine events is not yet a hundred"
    );

    // when: it is retitled once more
    let landed = ideas
        .retitle(&id.to_string(), "Title 100", None, &an_author())
        .await
        .expect("retitling should succeed")
        .version;

    // then: a snapshot follows the hundredth event
    assert_eq!(
        landed,
        Version::of(101),
        "a snapshot follows the hundredth event"
    );
}

#[tokio::test]
async fn an_idea_survives_on_its_snapshot_alone() {
    // given: an idea with a hundred events
    let Wired { ideas, store, .. } = wiring::wired(Arc::new(FixedClock::new(at(1_000))));
    let id = ideas
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed")
        .0;
    ideas
        .retitle(&id.to_string(), "Title 2", None, &an_author())
        .await
        .expect("retitling should succeed");

    for counted in 3..=100 {
        ideas
            .retitle(
                &id.to_string(),
                &format!("Title {counted}"),
                None,
                &an_author(),
            )
            .await
            .expect("retitling should succeed");
    }

    // and: everything before its snapshot pruned
    let key = AggregateId::from(&id);
    let snapshot = store
        .latest_snapshot(&key, KIND)
        .await
        .expect("looking should succeed")
        .expect("a hundred events should have earned a snapshot");
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    store
        .prune_through(
            &mut transaction,
            &key,
            KIND,
            snapshot
                .metadata
                .version
                .previous()
                .expect("a snapshot is never the first event"),
        )
        .await
        .expect("pruning should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // and: only the snapshot left
    assert_eq!(
        store
            .read_from(&key, KIND, Version::ZERO)
            .await
            .expect("reading should succeed")
            .len(),
        1,
        "everything the snapshot replaced is gone, so what follows can only come from it"
    );

    // when: it is fetched
    let idea = ideas
        .read(&id.to_string(), &an_author())
        .await
        .expect("the snapshot alone should be enough to rebuild the idea")
        .state;

    // then: it is the idea it was
    assert_eq!(idea.title().as_str(), "Title 100");
    assert_eq!(idea.project(), &ProjectLink::from("project_1"));
    assert!(!idea.is_discarded());
}

async fn an_idea_retitled_while_failing(
    failing: fn(Arc<InMemoryEventStore<IdeaEvent>>) -> FailsAppending<IdeaEvent>,
    expected: Option<Version>,
) -> (Result<Version, IdeasError>, String, u8) {
    let store = Arc::new(InMemoryEventStore::new());
    let core_over = |store| {
        IdeasCore::new(
            store,
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryIdeaCatalog::new()),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(at(1_000))),
        )
    };
    let id = a_captured_idea(&core_over(store.clone())).await;
    let failing = Arc::new(failing(store.clone()));

    let retitled = core_over(failing.clone())
        .retitle(&id.to_string(), "The Shuttle", expected, &an_author())
        .await
        .map(|standing| standing.version);
    let title = core_over(store)
        .read(&id.to_string(), &an_author())
        .await
        .expect("the idea exists")
        .state
        .title()
        .as_str()
        .to_owned();

    (retitled, title, failing.left())
}

#[tokio::test]
async fn a_retitling_that_lost_a_race_is_tried_again_and_lands() {
    // when: an idea is retitled over a store that loses four races
    let (retitled, title, _) =
        an_idea_retitled_while_failing(|store| FailsAppending::losing_races(store, 4), None).await;

    // then: the retitling lands
    assert_eq!(retitled.expect("the fifth try lands"), Version::of(2));
    assert_eq!(title, "The Shuttle");
}

#[tokio::test]
async fn a_retitling_that_lost_every_race_it_may_try_is_a_conflict() {
    // when: an idea is retitled over a store that loses six races
    let (retitled, title, left) =
        an_idea_retitled_while_failing(|store| FailsAppending::losing_races(store, 6), None).await;

    // then: it is a conflict
    assert!(matches!(
        retitled,
        Err(IdeasError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: the title stays after five tries
    assert_eq!(
        (title.as_str(), left),
        ("The Loom", 1),
        "five tries, no more"
    );
}

#[tokio::test]
async fn a_retitling_aimed_at_a_version_that_lost_a_race_is_not_tried_again() {
    // when: an idea is retitled at its version over a store that loses races
    let (retitled, title, left) = an_idea_retitled_while_failing(
        |store| FailsAppending::losing_races(store, 2),
        Some(Version::of(1)),
    )
    .await;

    // then: it is a conflict
    assert!(matches!(
        retitled,
        Err(IdeasError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: it was tried once
    assert_eq!(
        (title.as_str(), left),
        ("The Loom", 1),
        "whoever named a version hears that it moved on, a 412, rather than having it papered over"
    );
}

#[tokio::test]
async fn a_retitling_whose_store_broke_is_not_tried_again() {
    // when: an idea is retitled over a broken store
    let (retitled, _, left) =
        an_idea_retitled_while_failing(|store| FailsAppending::broken(store, 2), None).await;

    // then: the store's failure is told
    assert!(matches!(
        retitled,
        Err(IdeasError::Events(ServiceError::Store(
            StoreError::Backend { .. }
        )))
    ));
    // and: it was tried once
    assert_eq!(left, 1, "only a lost race is tried again");
}

#[tokio::test]
async fn a_retitling_aimed_at_the_version_it_stands_at_lands() {
    // when: an idea is retitled at the version it stands at
    let (retitled, title, _) = an_idea_retitled_while_failing(
        |store| FailsAppending::losing_races(store, 0),
        Some(Version::of(1)),
    )
    .await;

    // then: the retitling lands
    assert_eq!(
        (retitled.expect("retitling should succeed"), title.as_str()),
        (Version::of(2), "The Shuttle")
    );
}
