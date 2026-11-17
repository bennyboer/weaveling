use axum::Extension;
use axum_test::TestServer;
use eventsourcing::Agent;
use readings_contract::{BlockDTO, ContentsDTO, MarksDTO, PartDTO, RunDTO};
use readings_core::ReadingsCore;

fn server() -> TestServer {
    TestServer::new(readings_rest::router(ReadingsCore::new()).layer(Extension(Agent::Local)))
}

#[tokio::test]
async fn a_project_answers_with_its_book() {
    // given: the readings
    let server = server();

    // when: a project's live book is asked for
    let response = server.get("/readings/live?project=project_loom").await;

    // then: the book of that project comes back
    response.assert_status_ok();
    let book = response.json::<ContentsDTO>();
    assert_eq!(book.project, "project_loom");
    // and: its parts are sections and scenes in reading order
    let kinds: Vec<&str> = book
        .parts
        .iter()
        .map(|part| match part {
            PartDTO::Section { .. } => "section",
            PartDTO::Scene { .. } => "scene",
        })
        .collect();
    assert_eq!(kinds, ["section", "scene", "scene", "section", "scene"]);
}

#[tokio::test]
async fn a_scene_answers_with_its_blocks_and_marked_runs() {
    // given: the readings
    let server = server();

    // when: a project's live book is asked for
    let book = server
        .get("/readings/live?project=project_loom")
        .await
        .json::<ContentsDTO>();

    // then: a scene keeps the scene it came from and its text with its marks
    let PartDTO::Scene { scene, blocks, .. } = &book.parts[1] else {
        panic!("the second part is a scene");
    };
    assert_eq!(scene, "scene_harbour");
    assert_eq!(
        blocks[0],
        BlockDTO::Paragraph {
            runs: vec![
                RunDTO::Text {
                    text: "The boats came in before the light did, ".to_owned(),
                    marks: MarksDTO::default(),
                },
                RunDTO::Text {
                    text: "one by one".to_owned(),
                    marks: MarksDTO {
                        emphasis: true,
                        ..MarksDTO::default()
                    },
                },
                RunDTO::Text {
                    text: ", their lamps swinging.".to_owned(),
                    marks: MarksDTO::default(),
                },
            ],
        }
    );
}

#[tokio::test]
async fn asking_without_a_project_is_a_bad_request() {
    // given: the readings
    let server = server();

    // when: the live book is asked for without a project
    let response = server.get("/readings/live").expect_failure().await;

    // then: it is a bad request
    response.assert_status_bad_request();
}
