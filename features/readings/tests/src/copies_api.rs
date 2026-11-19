use std::collections::BTreeSet;
use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use readings_contract::{
    ExtentDTO, ListedReadingCopyDTO, MakeReadingCopyRequest, PartDTO, ReadingCopyDTO, SharedDTO,
    refused,
};
use readings_core::ReadingCopyId;
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};

use crate::copying::Copying;

const LOOM: &str = "project_loom";

/// Hedy owns the loom and Ada reads it; nobody else is a member of anything.
struct Members;

#[async_trait]
impl Authorizer for Members {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        Ok(project == LOOM
            && match agent {
                Agent::Account(account) if account.as_str() == "account_hedy" => true,
                Agent::Account(account) if account.as_str() == "account_ada" => act == Act::Read,
                _ => false,
            })
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Only(BTreeSet::from([LOOM.to_owned()])))
    }
}

struct Readings {
    copying: Copying,
    clock: Arc<FixedClock>,
}

fn started() -> OffsetDateTime {
    clock::at(1_700_000_000)
}

fn readings() -> Readings {
    let clock = Arc::new(FixedClock::new(started()));

    Readings {
        copying: crate::copying::a_workbench(
            KeptAccess::new(Arc::new(InMemoryAccess::new())),
            Arc::new(Members),
            clock.clone(),
        ),
        clock,
    }
}

fn account(named: &str) -> Agent {
    Agent::Account(AccountId::from(named))
}

impl Readings {
    fn as_(&self, agent: Agent) -> TestServer {
        TestServer::new(
            readings_rest::router(self.copying.readings.clone()).layer(Extension(agent)),
        )
    }

    fn hedy(&self) -> TestServer {
        self.as_(account("account_hedy"))
    }

    fn ada(&self) -> TestServer {
        self.as_(account("account_ada"))
    }

    fn later(&self, by: Duration) {
        self.clock.set(started() + by);
    }

    /// The project's copies as listed for whoever asks, once every change has reached the list.
    async fn listed(&self, server: &TestServer) -> Vec<ListedReadingCopyDTO> {
        self.copying.settle().await;
        let response = server
            .get(&format!("/readings/copies?project={LOOM}"))
            .await;
        response.assert_status_ok();

        response.json::<Vec<ListedReadingCopyDTO>>()
    }
}

fn asked_for(name: &str, shared: SharedDTO) -> MakeReadingCopyRequest {
    MakeReadingCopyRequest {
        project: LOOM.to_owned(),
        name: name.to_owned(),
        shared,
        open_until: Some("2023-12-14T00:00:00Z".to_owned()),
        deleted_on: Some("2024-02-14T00:00:00Z".to_owned()),
    }
}

fn both_sections() -> SharedDTO {
    SharedDTO::Sections {
        first: "section_morning".to_owned(),
        last: "section_market".to_owned(),
    }
}

async fn made(server: &TestServer, name: &str) -> ListedReadingCopyDTO {
    let response = server
        .post("/readings/copies")
        .json(&asked_for(name, both_sections()))
        .await;
    response.assert_status(StatusCode::CREATED);

    response.json::<ListedReadingCopyDTO>()
}

fn refusal(response: &TestResponse) -> Value {
    response.json::<Value>()["type"].clone()
}

#[tokio::test]
async fn a_copy_made_is_listed_for_every_member_as_it_was_made() {
    // given: the readings
    let readings = readings();

    // when: a reader makes a copy of both sections, open for a month and kept for three
    let made = made(&readings.ada(), "For Lena").await;

    // then: it answers as made by them, holding two sections, open, with its times
    assert_eq!(made.name, "For Lena");
    assert_eq!(made.extent, ExtentDTO::Sections { count: 2 });
    assert_eq!(made.open_until.as_deref(), Some("2023-12-14T00:00:00Z"));
    assert_eq!(made.deleted_on.as_deref(), Some("2024-02-14T00:00:00Z"));
    assert!(made.open);
    assert!(made.made_by_you);
    assert!(made.may_change);
    // and: they find it listed just so
    assert_eq!(readings.listed(&readings.ada()).await, vec![made.clone()]);
    // and: the owner finds it listed as made by someone else, though they may change it
    let [for_the_owner] = readings
        .listed(&readings.hedy())
        .await
        .try_into()
        .expect("one copy");
    assert_eq!(for_the_owner.id, made.id);
    assert!(!for_the_owner.made_by_you);
    assert!(for_the_owner.may_change);
}

#[tokio::test]
async fn a_project_s_copies_are_listed_newest_first() {
    // given: three copies made a day apart
    let readings = readings();
    for (day, name) in ["First", "Second", "Third"].into_iter().enumerate() {
        readings.later(Duration::days(day as i64));
        made(&readings.hedy(), name).await;
    }

    // when: the project's copies are asked for
    let names: Vec<String> = readings
        .listed(&readings.hedy())
        .await
        .into_iter()
        .map(|copy| copy.name)
        .collect();

    // then: the newest comes first
    assert_eq!(names, ["Third", "Second", "First"]);
}

#[tokio::test]
async fn a_copy_closes_once_its_opening_has_passed() {
    // given: a copy open for a month
    let readings = readings();
    made(&readings.hedy(), "For Lena").await;

    // when: two months pass
    readings.later(Duration::days(60));

    // then: it is listed as closed
    assert!(!readings.listed(&readings.hedy()).await[0].open);
}

#[tokio::test]
async fn a_copy_answers_with_what_it_holds() {
    // given: a copy of the project
    let readings = readings();
    let made = made(&readings.hedy(), "For Lena").await;

    // when: it is asked for
    let read = readings
        .hedy()
        .get(&format!("/readings/copies/{}", made.id))
        .await
        .json::<ReadingCopyDTO>();

    // then: it is the copy listed, with its work
    assert_eq!(read.copy, made);
    assert_eq!(read.contents.project, LOOM);
    assert!(matches!(read.contents.parts[0], PartDTO::Section { .. }));
}

#[tokio::test]
async fn a_copy_nobody_made_is_not_there() {
    // given: the readings
    let readings = readings();

    // expect: a copy nobody made, or that could be no copy at all, is not there
    for nobody_made in [
        ReadingCopyId::generate(started()).to_string(),
        "nonsense".to_owned(),
    ] {
        readings
            .hedy()
            .get(&format!("/readings/copies/{nobody_made}"))
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_copy_of_scenes_in_a_row_counts_them_across_sections() {
    // given: a copy asked for from the first scene of one section to the last of the next
    let readings = readings();
    let asked = asked_for(
        "Harbour to stalls",
        SharedDTO::Scenes {
            first: "scene_harbour".to_owned(),
            last: "scene_stalls".to_owned(),
        },
    );

    // when: it is made
    let response = readings.hedy().post("/readings/copies").json(&asked).await;

    // then: it holds the three scenes
    response.assert_status(StatusCode::CREATED);
    assert_eq!(
        response.json::<ListedReadingCopyDTO>().extent,
        ExtentDTO::Scenes { count: 3 }
    );
}

#[tokio::test]
async fn a_copy_of_what_is_not_in_the_work_is_refused() {
    // given: the readings
    let readings = readings();

    // expect: a section the work does not hold, or a run that ends before it begins, is refused
    for shared in [
        SharedDTO::Sections {
            first: "section_morning".to_owned(),
            last: "section_gone".to_owned(),
        },
        SharedDTO::Scenes {
            first: "scene_wall".to_owned(),
            last: "scene_harbour".to_owned(),
        },
    ] {
        let response = readings
            .hedy()
            .post("/readings/copies")
            .json(&asked_for("Nowhere", shared))
            .await;
        response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(refusal(&response), refused::OUTSIDE_THE_WORK);
    }
    // and: no copy was made
    assert_eq!(readings.listed(&readings.hedy()).await, vec![]);
}

#[tokio::test]
async fn a_copy_with_a_blank_name_or_open_beyond_its_deletion_is_refused() {
    // given: the readings
    let readings = readings();
    let mut open_too_long = asked_for("For Lena", both_sections());
    open_too_long.open_until = None;

    // expect: a blank name is refused
    let blank = readings
        .hedy()
        .post("/readings/copies")
        .json(&asked_for("   ", both_sections()))
        .await;
    blank.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&blank), refused::NAME_BLANK);
    // and: a copy open with no end, though it is deleted, is refused
    let beyond = readings
        .hedy()
        .post("/readings/copies")
        .json(&open_too_long)
        .await;
    beyond.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refusal(&beyond), refused::OPEN_BEYOND_DELETION);
}

#[tokio::test]
async fn a_moment_that_cannot_be_read_is_refused() {
    // given: a copy asked for with a moment that is none
    let mut asked = asked_for("For Lena", both_sections());
    asked.open_until = Some("next tuesday".to_owned());

    // when: it is asked for
    let response = readings()
        .hedy()
        .post("/readings/copies")
        .json(&asked)
        .await;

    // then: it is refused as unreadable
    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refusal(&response), refused::MOMENT_UNREADABLE);
}

#[tokio::test]
async fn a_copy_renamed_is_listed_under_its_new_name() {
    // given: a copy a reader made
    let readings = readings();
    let made = made(&readings.ada(), "For Lena").await;

    // when: they rename it
    let renamed = readings
        .ada()
        .put(&format!("/readings/copies/{}/name", made.id))
        .json(&json!({ "name": "For Lena, again" }))
        .await;

    // then: it answers and is listed under its new name
    renamed.assert_status_ok();
    assert_eq!(
        renamed.json::<ListedReadingCopyDTO>().name,
        "For Lena, again"
    );
    assert_eq!(
        readings.listed(&readings.hedy()).await[0].name,
        "For Lena, again"
    );
}

#[tokio::test]
async fn a_copy_s_times_move_later_and_never_earlier() {
    // given: a copy open for a month and kept for three
    let readings = readings();
    let made = made(&readings.hedy(), "For Lena").await;
    let times = format!("/readings/copies/{}/times", made.id);

    // when: it is opened until later, and kept for good
    let extended = readings
        .hedy()
        .put(&times)
        .json(&json!({ "open_until": "2024-01-14T00:00:00Z", "deleted_on": null }))
        .await;

    // then: it answers with its new times
    extended.assert_status_ok();
    let extended = extended.json::<ListedReadingCopyDTO>();
    assert_eq!(extended.open_until.as_deref(), Some("2024-01-14T00:00:00Z"));
    assert_eq!(extended.deleted_on, None);
    // and: moving its opening earlier again is refused
    let earlier = readings
        .hedy()
        .put(&times)
        .json(&json!({ "open_until": "2023-12-14T00:00:00Z", "deleted_on": null }))
        .await;
    earlier.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refusal(&earlier), refused::TIMES_EARLIER);
}

#[tokio::test]
async fn a_copy_deleted_is_gone() {
    // given: a copy
    let readings = readings();
    let made = made(&readings.hedy(), "For Lena").await;
    let copy = format!("/readings/copies/{}", made.id);

    // when: it is deleted
    readings
        .hedy()
        .delete(&copy)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is neither listed nor there, and deleting it again finds nothing
    assert_eq!(readings.listed(&readings.hedy()).await, vec![]);
    readings
        .hedy()
        .get(&copy)
        .await
        .assert_status(StatusCode::NOT_FOUND);
    readings
        .hedy()
        .delete(&copy)
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_late_message_about_a_copy_since_deleted_does_not_bring_it_back() {
    // given: a copy made, listed, and deleted
    let readings = readings();
    let made = made(&readings.hedy(), "For Lena").await;
    readings.listed(&readings.hedy()).await;
    readings
        .hedy()
        .delete(&format!("/readings/copies/{}", made.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    readings.copying.settle().await;

    // when: the message that it was made arrives once more
    readings
        .copying
        .readings
        .catalogue(
            None,
            &made
                .id
                .parse::<ReadingCopyId>()
                .expect("a copy's id reads back"),
        )
        .await
        .expect("cataloguing should succeed");

    // then: it stays gone, as the list is kept from the copy as it stands
    assert_eq!(readings.listed(&readings.hedy()).await, vec![]);
}
