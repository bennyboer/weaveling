use std::sync::Arc;

use authorizing::AllowingEverything;
use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::Agent;
use live::Broadcaster;
use live::testing::Subscriber;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_contract::{
    CreateSceneRequest, FRAGMENT, LinkIdeaRequest, QuotationRestylingDTO, RetitleSceneRequest,
    SceneDTO, ScenePageDTO, SceneSummaryDTO, refused,
};
use scenes_core::{
    Page, ProjectLink, QuotationRestylingTasks, SceneCatalog, SceneId, SceneStore, SceneTitle,
    ScenesCore,
};
use scenes_rest::{PAGE, TellingRestylingTasks};
use scenes_store::InMemorySceneStore;
use serde_json::Value;
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

use crate::committed::{Committed, TasksCommitted};

const UNKNOWN_ID: &str = "scene_031VkO0hnpeQZUiAB7nDma";

fn refused_as(response: TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");
    assert_eq!(told["status"], status.as_u16(), "{told}");

    told
}

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_core_of(
    store: Arc<dyn SceneStore>,
    catalog: Arc<dyn SceneCatalog>,
    tasks: Arc<dyn QuotationRestylingTasks>,
) -> ScenesCore {
    ScenesCore::new(
        store,
        Arc::new(InMemoryTransactions::new()),
        catalog,
        tasks,
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(at(1_700_000_000))),
    )
}

fn a_core_listing(catalog: Arc<dyn SceneCatalog>) -> ScenesCore {
    a_core_of(
        Arc::new(InMemorySceneStore::new()),
        catalog,
        Arc::new(InMemoryQuotationRestylingTasks::new()),
    )
}

fn a_core() -> ScenesCore {
    a_core_listing(Arc::new(InMemorySceneCatalog::new()))
}

fn a_server(scenes: ScenesCore) -> TestServer {
    TestServer::new(scenes_rest::router(scenes).layer(Extension(Agent::Local)))
}

fn a_paragraph(saying: &str) -> Vec<u8> {
    let doc = Doc::new();
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    {
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new(saying));
    }

    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

async fn a_scene(server: &TestServer) -> SceneDTO {
    let response = server
        .post("/scenes")
        .json(&CreateSceneRequest {
            project: A_PROJECT.to_owned(),
        })
        .await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

#[tokio::test]
async fn a_created_scene_is_reported_with_an_id_and_no_prose() {
    // given: scenes
    let server = a_server(a_core());

    // when: a scene is created
    let created = a_scene(&server).await;

    // then: it has an id and no prose
    assert!(!created.id.is_empty(), "a scene must be given an id");
    assert_eq!(created.text, "", "a new scene holds no prose");
}

#[tokio::test]
async fn two_created_scenes_are_distinct() {
    // given: scenes
    let server = a_server(a_core());

    // when: two scenes are created
    let one = a_scene(&server).await;
    let other = a_scene(&server).await;

    // then: their ids differ
    assert_ne!(one.id, other.id);
}

#[tokio::test]
async fn a_scene_can_be_read_back_by_its_id() {
    // given: a scene
    let server = a_server(a_core());
    let created = a_scene(&server).await;

    // when: it is read by its id
    let response = server.get(&format!("/scenes/{}", created.id)).await;

    // then: it is the scene that was created
    response.assert_status_ok();
    assert_eq!(response.json::<SceneDTO>(), created);
}

#[tokio::test]
async fn the_read_model_reports_the_prose_that_was_written() {
    // given: a scene
    let scenes = a_core();
    let server = a_server(scenes.clone());
    let created = a_scene(&server).await;
    // and: prose written into it
    scenes
        .apply(&created.id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should apply");

    // when: it is read
    let response = server.get(&format!("/scenes/{}", created.id)).await;

    // then: it shows the prose
    response.assert_status_ok();
    assert_eq!(
        response.json::<SceneDTO>().text,
        "The loom stood silent.",
        "the projection must show what the peers see"
    );
}

#[tokio::test]
async fn a_malformed_id_is_a_bad_request() {
    // given: scenes
    let server = a_server(a_core());

    // when: a scene is read by a malformed id
    let response = server.get("/scenes/weaveling").await;

    // then: it is refused as invalid
    refused_as(response, StatusCode::BAD_REQUEST, refused::ID_INVALID);
}

#[tokio::test]
async fn an_unknown_scene_is_not_found() {
    // given: scenes
    let server = a_server(a_core());

    // when: a scene nobody created is read
    let response = server.get(&format!("/scenes/{UNKNOWN_ID}")).await;

    // then: it is not found
    refused_as(response, StatusCode::NOT_FOUND, "not-found");
}

#[tokio::test]
async fn a_deleted_scene_is_gone() {
    // given: a scene
    let server = a_server(a_core());
    let created = a_scene(&server).await;

    // when: it is deleted
    server
        .delete(&format!("/scenes/{}", created.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is not found
    refused_as(
        server.get(&format!("/scenes/{}", created.id)).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn deleting_an_unknown_scene_is_not_found() {
    // given: scenes
    let server = a_server(a_core());

    // when: a scene nobody created is deleted
    let response = server.delete(&format!("/scenes/{UNKNOWN_ID}")).await;

    // then: it is not found
    refused_as(response, StatusCode::NOT_FOUND, "not-found");
}

async fn retitling(server: &TestServer, id: &str, title: &str) -> TestResponse {
    server
        .patch(&format!("/scenes/{id}"))
        .json(&RetitleSceneRequest {
            title: title.to_owned(),
        })
        .await
}

#[tokio::test]
async fn a_new_scene_is_reported_untitled() {
    // given: scenes
    let server = a_server(a_core());

    // when: a scene is created
    let created = a_scene(&server).await;

    // then: it has no title
    assert_eq!(
        created.title, "",
        "an untitled scene is named by its opening words"
    );
}

#[tokio::test]
async fn a_scene_can_be_retitled_and_read_back() {
    // given: a scene
    let server = a_server(a_core());
    let created = a_scene(&server).await;

    // when: it is retitled with a title padded by spaces
    let response = retitling(&server, &created.id, "  The loom  ").await;

    // then: it answers with the trimmed title
    response.assert_status_ok();
    assert_eq!(response.json::<SceneDTO>().title, "The loom");
    // and: reads back with it
    let found: SceneDTO = server.get(&format!("/scenes/{}", created.id)).await.json();
    assert_eq!(found.title, "The loom");
}

#[tokio::test]
async fn a_title_the_domain_refuses_is_unprocessable() {
    // given: a scene
    let server = a_server(a_core());
    let created = a_scene(&server).await;

    // when: it is retitled with a control character
    let response = retitling(&server, &created.id, "The\u{7}loom").await;

    // then: the title is refused as unprocessable
    refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TITLE_CONTROL_CHARACTER,
    );
}

#[tokio::test]
async fn retitling_a_scene_nobody_created_is_not_found() {
    // given: scenes
    let server = a_server(a_core());

    // when: a scene nobody created is retitled
    let response = retitling(&server, UNKNOWN_ID, "Nowhere").await;

    // then: it is not found
    refused_as(response, StatusCode::NOT_FOUND, "not-found");
}

async fn a_catalog_holding(count: i64) -> Arc<InMemorySceneCatalog> {
    let catalog = Arc::new(InMemorySceneCatalog::new());

    for made in 0..count {
        catalog
            .remember_committed(SceneId::generate(at(made)), &ProjectLink::from(A_PROJECT))
            .await
            .expect("remembering should succeed");
    }

    catalog
}

async fn page_after(server: &TestServer, after: Option<&str>) -> ScenePageDTO {
    let asked = match after {
        Some(last) => format!("/scenes?project={A_PROJECT}&after={last}"),
        None => format!("/scenes?project={A_PROJECT}"),
    };

    server.get(&asked).await.json()
}

#[tokio::test]
async fn a_listing_names_each_scene_from_the_catalog_without_its_text() {
    // given: a catalog holding a titled scene with an opening
    let catalog = a_catalog_holding(0).await;
    let scene = SceneId::generate(at(1_000));
    catalog
        .retitle_committed(
            scene,
            &ProjectLink::from(A_PROJECT),
            &SceneTitle::new("The loom").expect("a plain title"),
            1,
        )
        .await
        .expect("retitling should succeed");
    catalog
        .note_opening_committed(
            scene,
            &ProjectLink::from(A_PROJECT),
            "The loom stood silent.",
            at(1_000),
        )
        .await
        .expect("noting an opening should succeed");
    let server = a_server(a_core_listing(catalog));

    // when: the scenes are listed
    let listed = page_after(&server, None).await;

    // then: the scene is named by its title and opening, without its text
    assert_eq!(
        listed,
        ScenePageDTO {
            scenes: vec![SceneSummaryDTO {
                id: scene.to_string(),
                project: A_PROJECT.to_owned(),
                title: "The loom".to_owned(),
                opening: "The loom stood silent.".to_owned(),
            }],
            next: None,
        },
        "the outline names its leaves from this listing, so it must not open every scene"
    );
}

#[tokio::test]
async fn a_long_listing_comes_a_page_at_a_time() {
    // given: one scene more than a page holds
    let server = a_server(a_core_listing(a_catalog_holding(PAGE as i64 + 1).await));

    // when: the scenes are listed
    let first = page_after(&server, None).await;
    // and: listed again after the first page
    let rest = page_after(&server, first.next.as_deref()).await;

    // then: the first page is full and points past its last scene
    assert_eq!(first.scenes.len(), PAGE);
    assert_eq!(
        first.next.as_deref(),
        first.scenes.last().map(|scene| scene.id.as_str()),
        "the next page is asked for after the last scene of this one"
    );
    // and: the rest holds the one scene left, and no more
    assert_eq!(rest.scenes.len(), 1);
    assert_eq!(rest.next, None, "the last page says there is no more");
}

#[tokio::test]
async fn a_listing_exactly_one_page_long_says_there_is_no_more() {
    // given: exactly as many scenes as a page holds
    let server = a_server(a_core_listing(a_catalog_holding(PAGE as i64).await));

    // when: the scenes are listed
    let listed = page_after(&server, None).await;

    // then: they all come on one page that says there is no more
    assert_eq!(listed.scenes.len(), PAGE);
    assert_eq!(listed.next, None);
}

#[tokio::test]
async fn a_listing_after_something_that_is_no_scene_is_a_bad_request() {
    // given: scenes
    let server = a_server(a_core());

    // when: the scenes after something that is no scene are listed
    let response = server
        .get(&format!("/scenes?project={A_PROJECT}&after=idea_1"))
        .await;

    // then: it is refused as invalid
    refused_as(response, StatusCode::BAD_REQUEST, refused::ID_INVALID);
}

async fn linking(server: &TestServer, id: &str, idea: &str) -> axum_test::TestResponse {
    server
        .post(&format!("/scenes/{id}/ideas"))
        .json(&LinkIdeaRequest {
            idea: idea.to_owned(),
        })
        .await
}

#[tokio::test]
async fn a_new_scene_has_no_linked_ideas() {
    // given: scenes
    let server = a_server(a_core());

    // when: a scene is created
    let created = a_scene(&server).await;

    // then: it has no ideas linked
    assert!(created.ideas.is_empty());
}

#[tokio::test]
async fn an_idea_can_be_linked_and_unlinked() {
    // given: a scene
    let server = a_server(a_core());
    let created = a_scene(&server).await;

    // when: an idea is linked to it
    let after_linking = linking(&server, &created.id, "idea_1").await;
    // then: it holds the idea
    after_linking.assert_status_ok();
    assert_eq!(after_linking.json::<SceneDTO>().ideas, vec!["idea_1"]);

    // when: the idea is unlinked
    let after_unlinking = server
        .delete(&format!("/scenes/{}/ideas/idea_1", created.id))
        .await;
    // then: it holds no ideas
    after_unlinking.assert_status_ok();
    assert!(after_unlinking.json::<SceneDTO>().ideas.is_empty());
}

#[tokio::test]
async fn linking_to_a_scene_nobody_created_is_not_found() {
    // given: scenes
    let server = a_server(a_core());

    // then: linking an idea to a scene nobody created is not found
    refused_as(
        linking(&server, UNKNOWN_ID, "idea_1").await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
    // and: unlinking one is not found either
    refused_as(
        server
            .delete(&format!("/scenes/{UNKNOWN_ID}/ideas/idea_1"))
            .await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

async fn asking_how_far_restyled(
    tasks: InMemoryQuotationRestylingTasks,
) -> axum_test::TestResponse {
    TestServer::new(
        scenes_rest::quotation_restylings(a_core_of(
            Arc::new(InMemorySceneStore::new()),
            Arc::new(InMemorySceneCatalog::new()),
            Arc::new(tasks),
        ))
        .layer(Extension(Agent::Local)),
    )
    .get(&format!("/scenes/quotation-restylings/{A_PROJECT}"))
    .await
}

struct RestylingFollowed {
    tasks: Arc<dyn QuotationRestylingTasks>,
    socket: Subscriber,
    _server: TestServer,
}

async fn a_restyling_followed(project: &str) -> RestylingFollowed {
    let broadcaster = Broadcaster::new();
    let tasks: Arc<dyn QuotationRestylingTasks> = Arc::new(TellingRestylingTasks::around(
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        broadcaster.clone(),
    ));
    let scenes = a_core_of(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemorySceneCatalog::new()),
        tasks.clone(),
    );
    let server = TestServer::builder().http_transport().build(
        live::live_socket(live::Topics::of(vec![scenes_rest::restylings_topic(
            scenes,
            broadcaster,
        )]))
        .layer(Extension(Agent::Local)),
    );
    let mut socket = Subscriber::on(&server).await;
    socket
        .subscribe(
            "restylings",
            "quotation-restylings",
            &[("project", project)],
        )
        .await;

    RestylingFollowed {
        tasks,
        socket,
        _server: server,
    }
}

async fn told_progress(socket: &mut Subscriber) -> QuotationRestylingDTO {
    serde_json::from_value(socket.frame().await["event"].clone())
        .expect("the frame carries the progress")
}

#[tokio::test]
async fn whoever_follows_a_book_s_restyling_is_told_as_it_begins_and_as_it_goes() {
    // given: a book's restyling followed
    let mut followed = a_restyling_followed(A_PROJECT).await;
    let project = ProjectLink::from(A_PROJECT);

    // when: its restyling begins
    followed
        .tasks
        .begin_committed(&project, 3, 5)
        .await
        .expect("beginning should succeed");
    let begun = told_progress(&mut followed.socket).await;
    // and: goes all the way
    followed
        .tasks
        .progressed_committed(
            &project,
            3,
            Page {
                from: None,
                to: None,
                done: 5,
            },
            true,
        )
        .await
        .expect("counting should succeed");
    let finished = told_progress(&mut followed.socket).await;

    // then: the follower is told it has begun with nothing done
    assert_eq!(
        begun,
        QuotationRestylingDTO {
            change: 3,
            total: 5,
            done: 0,
            finished: false,
        }
    );
    // and: that it has finished with all done
    assert_eq!(
        finished,
        QuotationRestylingDTO {
            change: 3,
            total: 5,
            done: 5,
            finished: true,
        }
    );
}

#[tokio::test]
async fn an_older_change_that_cannot_begin_and_another_book_tell_nothing() {
    // given: a book's restyling followed, as it begins
    let mut followed = a_restyling_followed(A_PROJECT).await;
    let project = ProjectLink::from(A_PROJECT);
    followed
        .tasks
        .begin_committed(&project, 3, 5)
        .await
        .expect("beginning should succeed");
    told_progress(&mut followed.socket).await;

    // when: a restyling for an older change begins
    followed
        .tasks
        .begin_committed(&project, 2, 9)
        .await
        .expect("an older change is turned away, not failed");
    // and: another book's restyling begins
    followed
        .tasks
        .begin_committed(&ProjectLink::from("project_other"), 1, 1)
        .await
        .expect("beginning should succeed");

    // then: the follower is told nothing
    followed.socket.nothing().await;
}

#[tokio::test]
async fn how_far_a_book_is_restyled_can_be_asked() {
    // given: a book's restyling partly done
    let tasks = InMemoryQuotationRestylingTasks::new();
    let project = ProjectLink::from(A_PROJECT);
    tasks
        .begin_committed(&project, 3, 5)
        .await
        .expect("beginning should succeed");
    tasks
        .progressed_committed(
            &project,
            3,
            Page {
                from: None,
                to: None,
                done: 2,
            },
            false,
        )
        .await
        .expect("counting should succeed");

    // when: how far it is restyled is asked
    let response = asking_how_far_restyled(tasks).await;

    // then: it tells how much is done of how much
    response.assert_status_ok();
    assert_eq!(
        response.json::<QuotationRestylingDTO>(),
        QuotationRestylingDTO {
            change: 3,
            total: 5,
            done: 2,
            finished: false,
        }
    );
}

#[tokio::test]
async fn a_book_never_restyled_is_not_found() {
    // expect: how far a book never restyled is restyled is not found
    refused_as(
        asking_how_far_restyled(InMemoryQuotationRestylingTasks::new()).await,
        StatusCode::NOT_FOUND,
        refused::NEVER_RESTYLED,
    );
}

#[tokio::test]
async fn more_done_than_was_counted_is_told_as_all_of_it() {
    // given: a book's restyling with more done than was counted
    let tasks = InMemoryQuotationRestylingTasks::new();
    let project = ProjectLink::from(A_PROJECT);
    tasks
        .begin_committed(&project, 3, 2)
        .await
        .expect("beginning should succeed");
    tasks
        .progressed_committed(
            &project,
            3,
            Page {
                from: None,
                to: None,
                done: 3,
            },
            true,
        )
        .await
        .expect("counting should succeed");

    // when: how far it is restyled is asked
    let told = asking_how_far_restyled(tasks)
        .await
        .json::<QuotationRestylingDTO>();

    // then: all of it is told as done, and no more
    assert_eq!(
        told.done, 2,
        "a scene written while the book was restyled, or a batch delivered twice, \
         must not take the bar past its end"
    );
}

#[tokio::test]
async fn a_title_too_long_is_refused_telling_how_long_it_may_be() {
    // given: a scene
    let server = a_server(a_core());
    let created = a_scene(&server).await;

    // when: it is retitled one character too long
    let response = retitling(&server, &created.id, &"a".repeat(SceneTitle::MAX_CHARS + 1)).await;

    // then: the title is refused, telling the most it may be and what it was
    let told = refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TITLE_TOO_LONG,
    );
    assert_eq!(told["max"], SceneTitle::MAX_CHARS);
    assert_eq!(told["actual"], SceneTitle::MAX_CHARS + 1);
}
