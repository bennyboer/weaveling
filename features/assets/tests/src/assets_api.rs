use assets_contract::{AssetDTO, OwnerDTO, UsageDTO, refused};
use assets_core::{ImageRefused, Limits};
use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG};
use axum_test::{TestResponse, TestServer};
use eventsourcing::Agent;
use serde_json::Value;

use crate::bench::{self, Bench, THE_LOOM};

const UNKNOWN_ID: &str = "asset_031VkO0hnpeQZUiAB7nDma";

fn serving(bench: &Bench, acting: Agent) -> TestServer {
    TestServer::new(assets_rest::router(bench.assets.clone()).layer(Extension(acting)))
}

fn refused_as(response: TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");

    told
}

async fn uploaded(server: &TestServer, project: &str) -> TestResponse {
    server
        .post(&format!("/assets?project={project}"))
        .bytes(b"an image of the loom".to_vec().into())
        .await
}

#[tokio::test]
async fn an_upload_into_a_project_is_answered_with_what_was_made_of_it() {
    // given: Ada, an owner of the loom
    let bench = bench::a_bench();
    let server = serving(&bench, bench::ada());

    // when: she uploads an image into it
    let response = uploaded(&server, THE_LOOM).await;

    // then: it is created, versioned, with the picture and the widths it can be had in
    response.assert_status(StatusCode::CREATED);
    assert_eq!(response.header(ETAG), "\"1\"");
    let asset: AssetDTO = response.json();
    assert!(asset.id.starts_with("asset_"), "{}", asset.id);
    assert_eq!(asset.owner, OwnerDTO::Project(THE_LOOM.to_owned()));
    assert_eq!(asset.format, "jpeg");
    assert_eq!((asset.width, asset.height), (1_280, 1_024));
    assert_eq!(asset.widths, vec![384, 768, 1_280]);
    assert_eq!(asset.original_size, 20);
    assert_eq!(asset.version, 1);
}

#[tokio::test]
async fn the_content_is_the_variant_asked_for_kept_by_the_browser_for_good() {
    // given: an image uploaded into the loom
    let bench = bench::a_bench();
    let server = serving(&bench, bench::ada());
    let asset: AssetDTO = uploaded(&server, THE_LOOM).await.json();

    // when: a reader of the loom asks for it 500 pixels wide, and for it as wide as it comes
    let reading = serving(&bench, bench::bo());
    let narrow = reading
        .get(&format!("/assets/{}/content?width=500", asset.id))
        .await;
    let widest = reading.get(&format!("/assets/{}/content", asset.id)).await;

    // then: the narrowest at least as wide comes, and the widest unasked, as WebP kept privately for good
    narrow.assert_status_ok();
    assert_eq!(narrow.as_bytes().as_ref(), b"the 768 wide variant");
    assert_eq!(narrow.header(CONTENT_TYPE), "image/webp");
    assert_eq!(
        narrow.header(CACHE_CONTROL),
        "private, max-age=31536000, immutable"
    );
    assert_eq!(widest.as_bytes().as_ref(), b"the 1280 wide variant");
}

#[tokio::test]
async fn a_reader_may_not_upload_and_someone_outside_the_project_finds_nothing() {
    // given: Bo, a reader of the loom, and Grace, no member of it
    let bench = bench::a_bench();
    let asset: AssetDTO = uploaded(&serving(&bench, bench::ada()), THE_LOOM)
        .await
        .json();

    // when: Bo uploads, and Grace uploads and asks for the image
    let by_a_reader = uploaded(&serving(&bench, bench::bo()), THE_LOOM).await;
    let outside = serving(&bench, bench::grace());
    let by_someone_outside = uploaded(&outside, THE_LOOM).await;
    let looked_at = outside.get(&format!("/assets/{}/content", asset.id)).await;

    // then: the reader is told reading is all they may, and to Grace there is neither project nor image
    refused_as(by_a_reader, StatusCode::FORBIDDEN, refused::NOT_AN_AUTHOR);
    refused_as(
        by_someone_outside,
        StatusCode::NOT_FOUND,
        serving::NOT_FOUND,
    );
    refused_as(looked_at, StatusCode::NOT_FOUND, serving::NOT_FOUND);
}

#[tokio::test]
async fn an_upload_that_is_no_image_says_so() {
    // given: images refused as none
    let bench = bench::a_bench_refusing(ImageRefused::NotAnImage);

    // when: something is uploaded
    let response = uploaded(&serving(&bench, bench::ada()), THE_LOOM).await;

    // then: it is refused as no image
    refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::NOT_AN_IMAGE,
    );
}

#[tokio::test]
async fn an_upload_past_its_limit_is_refused_before_it_is_read_naming_the_limit() {
    // given: uploads of ten bytes at most
    let bench = bench::a_bench_limited(Limits {
        each_upload: Some(10),
        ..Limits::default()
    });

    // when: twenty bytes are uploaded
    let response = uploaded(&serving(&bench, bench::ada()), THE_LOOM).await;

    // then: the upload's limit is named, and nothing was made of it
    let told = refused_as(
        response,
        StatusCode::PAYLOAD_TOO_LARGE,
        refused::LIMIT_REACHED,
    );
    assert_eq!(told["bound"], "upload");
    assert_eq!(told["most"], 10);
    assert_eq!(bench.images.asked(), 0);
}

#[tokio::test]
async fn an_upload_past_what_a_project_may_keep_names_the_project_s_limit_and_what_it_holds() {
    // given: a project that may keep a hundred bytes, holding one image already
    let bench = bench::a_bench_limited(Limits {
        each_project: Some(100),
        ..Limits::default()
    });
    let server = serving(&bench, bench::ada());
    uploaded(&server, THE_LOOM)
        .await
        .assert_status(StatusCode::CREATED);
    let held = bench.usage().await.project;

    // when: another is uploaded
    let response = uploaded(&server, THE_LOOM).await;

    // then: the project's limit is named, with what it holds
    let told = refused_as(
        response,
        StatusCode::PAYLOAD_TOO_LARGE,
        refused::LIMIT_REACHED,
    );
    assert_eq!(told["bound"], "project");
    assert_eq!(told["most"], 100);
    assert_eq!(told["used"], held);
}

#[tokio::test]
async fn a_deleted_image_is_found_no_more() {
    // given: an image uploaded into the loom
    let bench = bench::a_bench();
    let server = serving(&bench, bench::ada());
    let asset: AssetDTO = uploaded(&server, THE_LOOM).await.json();

    // when: it is deleted
    let response = server.delete(&format!("/assets/{}", asset.id)).await;

    // then: it is gone
    response.assert_status(StatusCode::NO_CONTENT);
    refused_as(
        server.get(&format!("/assets/{}/content", asset.id)).await,
        StatusCode::NOT_FOUND,
        serving::NOT_FOUND,
    );
}

#[tokio::test]
async fn an_id_that_is_none_is_refused_and_one_never_given_is_not_found() {
    // given: Ada
    let bench = bench::a_bench();
    let server = serving(&bench, bench::ada());

    // when: an image is asked for by no id and by one never given
    let unreadable = server.get("/assets/not-an-id/content").await;
    let unknown = server.get(&format!("/assets/{UNKNOWN_ID}/content")).await;

    // then: the one is refused, the other not found
    refused_as(unreadable, StatusCode::BAD_REQUEST, refused::ID_INVALID);
    refused_as(unknown, StatusCode::NOT_FOUND, serving::NOT_FOUND);
}

#[tokio::test]
async fn an_upload_is_taken_up_to_its_limit_however_far_past_what_a_request_takes_unless_told() {
    // given: uploads of three megabytes at most, and with no limit at all
    let limited = bench::a_bench_limited(Limits {
        each_upload: Some(3 * 1024 * 1024),
        ..Limits::default()
    });
    let unlimited = bench::a_bench();
    let large = vec![7; 5 * 1024 * 1024 / 2];

    // when: two and a half megabytes are uploaded into each
    let into_the_limited = serving(&limited, bench::ada())
        .post(&format!("/assets?project={THE_LOOM}"))
        .bytes(large.clone().into())
        .await;
    let into_the_unlimited = serving(&unlimited, bench::ada())
        .post(&format!("/assets?project={THE_LOOM}"))
        .bytes(large.into())
        .await;

    // then: both are taken
    into_the_limited.assert_status(StatusCode::CREATED);
    into_the_unlimited.assert_status(StatusCode::CREATED);
}

#[tokio::test]
async fn an_upload_into_no_project_is_the_account_s_own_and_refused_without_one() {
    // given: Ada, and the instance without accounts
    let bench = bench::a_bench();

    // when: she uploads into no project, and so does the local agent
    let hers = serving(&bench, bench::ada())
        .post("/assets")
        .bytes(b"a portrait".to_vec().into())
        .await;
    let local = serving(&bench, Agent::Local)
        .post("/assets")
        .bytes(b"a portrait".to_vec().into())
        .await;

    // then: hers is her own, and without an account it is refused
    hers.assert_status(StatusCode::CREATED);
    let asset: AssetDTO = hers.json();
    assert_eq!(asset.owner, OwnerDTO::Account("account_ada".to_owned()));
    refused_as(local, StatusCode::FORBIDDEN, refused::NO_ACCOUNT);
}

#[tokio::test]
async fn an_account_is_told_what_its_uploads_take_wherever_they_went_and_the_most_they_may() {
    // given: every account held to a megabyte, Ada given two of her own
    let bench = bench::a_bench_limited(Limits {
        each_account: Some(1 << 20),
        ..Limits::default()
    });
    bench.adas_own_limit(Some(2 << 20)).await;
    // and: an image of hers in the loom and one of her own
    let server = serving(&bench, bench::ada());
    let into_the_loom: AssetDTO = uploaded(&server, THE_LOOM).await.json();
    let her_own: AssetDTO = server
        .post("/assets")
        .bytes(b"a portrait".to_vec().into())
        .await
        .json();

    // when: she and Bo ask what their uploads take
    let hers: UsageDTO = server.get("/assets/usage").await.json();
    let his: UsageDTO = serving(&bench, bench::bo())
        .get("/assets/usage")
        .await
        .json();

    // then: hers take both images, out of her own two megabytes, and his nothing, out of every account's one
    assert_eq!(
        hers,
        UsageDTO {
            used: into_the_loom.total_size + her_own.total_size,
            most: Some(2 << 20),
        }
    );
    assert_eq!(
        his,
        UsageDTO {
            used: 0,
            most: Some(1 << 20),
        }
    );
}

#[tokio::test]
async fn without_an_account_there_is_no_usage_of_one_s_own() {
    // given: nobody signed in, as without accounts
    let bench = bench::a_bench();

    // when: what one's uploads take is asked
    let response = serving(&bench, Agent::Local).get("/assets/usage").await;

    // then: it is refused, as nobody keeps assets of their own
    refused_as(response, StatusCode::FORBIDDEN, refused::NO_ACCOUNT);
}
