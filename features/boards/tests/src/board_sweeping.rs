use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ::wiring::Context;
use async_trait::async_trait;
use boards_catalog::InMemoryBoardCatalog;
use boards_core::{
    BoardCatalog, BoardEvent, BoardId, BoardSummary, BoardsError, CatalogError, IdeaLink, KIND,
    ProjectLink,
};
use boards_messaging::{
    DiscardBoardsOnProjectDeletedListener, RememberDeletedProjectsListener, SWEEP_PROJECT,
    SweepProjectDTO,
};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, Version};
use inbox::{InMemoryInbox, Inboxed};
use messaging::{FakePublisher, Listener, Message, RoutingKey};
use outbox::InMemoryOutbox;
use projects_contract::DELETED;
use registry::{InMemoryRegistry, Registry};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use transacting::{InMemoryTransactions, Transaction, Transactions};

use crate::wiring;
use crate::wiring::{TheSystemAndTheAuthor, Wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::Account(AccountId::from("author-7"))
}

fn a_workbench() -> (Wired, DiscardBoardsOnProjectDeletedListener) {
    let wired = wiring::wired_guarded_by(
        Arc::new(FixedClock::new(at(1_000))),
        Arc::new(TheSystemAndTheAuthor::default()),
    );
    let sweep = DiscardBoardsOnProjectDeletedListener::new(wired.boards.clone());

    (wired, sweep)
}

fn asked_to_sweep(project: &str) -> Message {
    Message::opening(
        RoutingKey::parse(SWEEP_PROJECT).expect("a follow-up's routing key holds no wildcards"),
        json!(SweepProjectDTO {
            project: project.to_owned(),
        }),
        at(1_000),
    )
}

#[tokio::test]
async fn a_board_is_discarded_in_the_system_s_name() {
    let (wired, sweep) = a_workbench();
    let opened = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;

    sweep
        .handle(&asked_to_sweep("project_1"))
        .await
        .expect("sweeping should succeed");

    let recorded = wired
        .store
        .read_from(&AggregateId::from(&opened), KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(
        recorded
            .last()
            .expect("the discard is recorded")
            .metadata
            .agent,
        Agent::System,
        "the system carries out what the deletion decided, and says so"
    );
}

#[tokio::test]
async fn a_board_the_catalog_has_not_heard_of_yet_is_still_swept() {
    let (wired, sweep) = a_workbench();
    let opened = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    assert!(
        wired
            .catalog
            .in_project(&"project_1".into())
            .await
            .expect("asking should succeed")
            .is_empty(),
        "nothing has relayed the start yet, so the catalog cannot know this board"
    );

    sweep
        .handle(&asked_to_sweep("project_1"))
        .await
        .expect("the sweep should take the deletion");

    let standing = wired
        .boards
        .read(&opened.to_string(), &an_author())
        .await
        .expect("the board is still there to read");
    assert!(
        standing.state.is_discarded(),
        "a board opened just before its project was deleted must not outlive it"
    );
}

#[tokio::test]
async fn a_project_that_never_opened_a_board_is_swept_without_complaint() {
    let (_wired, sweep) = a_workbench();

    sweep
        .handle(&asked_to_sweep("project_1"))
        .await
        .expect("nothing to discard is not a failure");
}

#[tokio::test]
async fn hearing_the_same_deletion_twice_is_harmless() {
    let (wired, sweep) = a_workbench();
    wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed");

    for _ in 0..2 {
        sweep
            .handle(&asked_to_sweep("project_1"))
            .await
            .expect("a redelivered deletion should be taken again");
    }
}

#[tokio::test]
async fn only_the_deleted_project_loses_its_board() {
    let (wired, sweep) = a_workbench();
    let kept = wired
        .boards
        .open("project_2", &an_author())
        .await
        .expect("opening should succeed")
        .id;

    sweep
        .handle(&asked_to_sweep("project_1"))
        .await
        .expect("the sweep should take the deletion");

    assert!(
        !wired
            .boards
            .read(&kept.to_string(), &an_author())
            .await
            .expect("the other board is there to read")
            .state
            .is_discarded()
    );
}

#[tokio::test]
async fn a_swept_board_leaves_the_catalog() {
    let (wired, sweep) = a_workbench();
    wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed");
    wired.settle().await;
    let listed = || async {
        wired
            .catalog
            .in_project(&"project_1".into())
            .await
            .expect("asking should succeed")
            .len()
    };
    assert_eq!(
        listed().await,
        1,
        "the start should have reached the catalog"
    );

    sweep
        .handle(&asked_to_sweep("project_1"))
        .await
        .expect("the sweep should take the deletion");
    wired.settle().await;

    assert_eq!(listed().await, 0, "a discarded board must not stay listed");
}

fn deleted(project: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DELETED" },
            "aggregate": { "id": project, "kind": "project", "version": 3 },
            "agent": { "kind": "local", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

fn remembering(
    wired: &Wired,
    hearings: InMemoryTransactions,
) -> Inboxed<RememberDeletedProjectsListener> {
    Inboxed::new(
        RememberDeletedProjectsListener::new(
            wired.boards.clone(),
            wired.outbox.clone(),
            wired.clock.clone(),
        ),
        Arc::new(hearings),
        Arc::new(InMemoryInbox::new(wired.clock.clone())),
    )
}

async fn buried(wired: &Wired, project: &str) -> bool {
    wired
        .catalog
        .has_project_tombstone(&ProjectLink::from(project))
        .await
        .expect("asking should succeed")
}

#[tokio::test]
async fn a_deletion_lays_the_project_s_tombstone_and_asks_for_the_sweep() {
    let (wired, _sweep) = a_workbench();

    remembering(&wired, InMemoryTransactions::new())
        .handle(&deleted("project_1"))
        .await
        .expect("the deletion should be heard");

    assert!(buried(&wired, "project_1").await);
    assert_eq!(
        wired.outbox.waiting_to_be_published(),
        1,
        "the sweep is asked for in the tombstone's transaction, so it reads only what came before"
    );
}

#[tokio::test]
async fn a_deletion_heard_in_a_hearing_that_does_not_commit_lays_nothing_and_asks_nothing() {
    let (wired, _sweep) = a_workbench();

    remembering(&wired, InMemoryTransactions::refusing_commits())
        .handle(&deleted("project_1"))
        .await
        .expect_err("a hearing that does not commit is not handled");

    assert!(!buried(&wired, "project_1").await);
    assert_eq!(wired.outbox.waiting_to_be_published(), 0);
}

#[tokio::test]
async fn a_deleted_project_opens_no_board() {
    let (wired, _sweep) = a_workbench();
    wired
        .boards
        .lay_project_tombstone(None, "project_1", at(900))
        .await
        .expect("laying a project's tombstone should succeed");

    let refused = wired.boards.open("project_1", &an_author()).await;

    assert!(matches!(refused, Err(BoardsError::ProjectNotThere)));
    assert_eq!(
        wired
            .registry
            .holder(KIND.as_str(), "project_1")
            .await
            .expect("asking should succeed"),
        None,
        "nothing is claimed for a project that is gone"
    );
}

#[tokio::test]
async fn a_board_opened_before_the_deletion_is_discarded_once_the_sweep_it_asks_for_comes() {
    let (wired, _sweep) = a_workbench();
    let opened = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    wired.settle().await;

    remembering(&wired, InMemoryTransactions::new())
        .handle(&deleted("project_1"))
        .await
        .expect("the deletion should be heard");
    wired.settle().await;

    assert!(
        wired
            .boards
            .read(&opened.to_string(), &an_author())
            .await
            .expect("the board is still there to read")
            .state
            .is_discarded(),
        "the sweep reads the registry after the tombstone committed, so it finds the board"
    );
}

struct BuriedMidOpen {
    catalog: Arc<InMemoryBoardCatalog>,
    asked: AtomicUsize,
}

#[async_trait]
impl BoardCatalog for BuriedMidOpen {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &BoardSummary,
    ) -> Result<(), CatalogError> {
        self.catalog.remember(transaction, summary).await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        board: &BoardId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.catalog.forget(transaction, board, at).await
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<BoardSummary>, CatalogError> {
        self.catalog.in_project(project).await
    }

    async fn holds(
        &self,
        transaction: &mut Transaction,
        board: BoardId,
        ideas: &[IdeaLink],
    ) -> Result<(), CatalogError> {
        self.catalog.holds(transaction, board, ideas).await
    }

    async fn boards_holding(&self, idea: &IdeaLink) -> Result<Vec<BoardId>, CatalogError> {
        self.catalog.boards_holding(idea).await
    }

    async fn lay_tombstone(
        &self,
        transaction: &mut Transaction,
        idea: &IdeaLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.catalog.lay_tombstone(transaction, idea, at).await
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.catalog
            .forget_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    async fn held_though_tombstoned(&self, board: BoardId) -> Result<Vec<IdeaLink>, CatalogError> {
        self.catalog.held_though_tombstoned(board).await
    }

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.catalog
            .lay_project_tombstone(transaction, project, at)
            .await
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        if self.asked.fetch_add(1, Ordering::AcqRel) == 0 {
            return Ok(false);
        }

        self.catalog.has_project_tombstone(project).await
    }
}

#[tokio::test]
async fn a_tombstone_laid_while_the_board_was_opened_discards_what_the_open_started() {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let store = Arc::new(InMemoryEventStore::new());
    let registry = Arc::new(InMemoryRegistry::new());
    let catalog = Arc::new(InMemoryBoardCatalog::new());
    let transactions = Arc::new(InMemoryTransactions::new());
    let mut transaction = transactions
        .begin()
        .await
        .expect("an in-memory transaction begins");
    catalog
        .lay_project_tombstone(&mut transaction, &ProjectLink::from("project_1"), at(900))
        .await
        .expect("laying a project's tombstone should succeed");
    transaction
        .finish()
        .await
        .expect("an in-memory transaction commits");
    let boards = boards_wiring::core(
        &boards_wiring::Ports {
            events: store.clone(),
            transactions,
            catalog: Arc::new(BuriedMidOpen {
                catalog,
                asked: AtomicUsize::new(0),
            }),
            registry: registry.clone(),
            inbox: Arc::new(InMemoryInbox::new(clock.clone())),
            outbox: Arc::new(InMemoryOutbox::new(
                Arc::new(FakePublisher::default()),
                clock.clone(),
            )),
        },
        &Context {
            clock,
            publisher: Arc::new(FakePublisher::default()),
            authorizer: Arc::new(TheSystemAndTheAuthor::default()),
        },
    );

    let refused = boards.open("project_1", &an_author()).await;

    let started: BoardId = registry
        .holder(KIND.as_str(), "project_1")
        .await
        .expect("asking should succeed")
        .expect("the open claimed before it saw the tombstone")
        .parse()
        .expect("the registry holds an id");
    let happened: Vec<BoardEvent> = store
        .read_from(&AggregateId::from(&started), KIND, Version::ZERO)
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| recorded.event)
        .collect();
    assert!(matches!(refused, Err(BoardsError::ProjectNotThere)));
    assert!(
        matches!(happened.last(), Some(BoardEvent::Discarded)),
        "an open that finds the tombstone after its own commit discards what it started, so the board does not outlive the project"
    );
}
