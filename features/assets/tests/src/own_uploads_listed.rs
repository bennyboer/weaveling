use access::{HearsAccess, Role};
use assets_contract::{OwnerDTO, UploadsDTO, refused};
use assets_core::{AssetId, AssetsError, ProjectLink, UploadsSorted};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use eventsourcing::{AccountId, Agent, Version};
use serde_json::Value;

use crate::bench::{self, Bench, THE_LOOM};

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";

/// Ada uploads a photo into the loom and a portrait of her own; the loom has a name.
async fn ada_having_uploaded() -> (Bench, AssetId, AssetId) {
    let bench = bench::a_bench().await;
    let (into_the_loom, _) = bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an author uploads");
    let (her_own, _) = bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an account uploads its own");
    bench
        .assets
        .remember_project_name(None, &ProjectLink::from(THE_LOOM), Version::of(1), "Loom")
        .await
        .expect("remembering should succeed");

    (bench, into_the_loom, her_own)
}

/// Demotes Ada to a reader of the loom.
async fn ada_only_reads_the_loom(bench: &Bench) {
    bench
        .assets
        .remember_role(
            None,
            THE_LOOM,
            &AccountId::from("account_ada"),
            Version::of(3),
            Some(Role::Reader),
        )
        .await
        .expect("remembering should succeed");
}

fn serving(bench: &Bench, acting: Agent) -> TestServer {
    TestServer::new(assets_rest::router(bench.assets.clone()).layer(Extension(acting)))
}

#[tokio::test]
async fn an_account_is_told_everything_it_uploaded_and_that_it_may_delete_it() {
    // given: Ada with a photo in the loom and a portrait of her own, Bo with nothing
    let (bench, into_the_loom, her_own) = ada_having_uploaded().await;

    // when: each lists their uploads
    let hers = bench
        .assets
        .uploads_of(UploadsSorted::Newest, None, 10, &bench::ada())
        .await
        .expect("an account is told its uploads");
    let his = bench
        .assets
        .uploads_of(UploadsSorted::Newest, None, 10, &bench::bo())
        .await
        .expect("an account is told its uploads");

    // then: Ada's two, wherever they went, each hers to delete
    let mut listed: Vec<AssetId> = hers.listed.iter().map(|own| own.upload.id).collect();
    listed.sort();
    let mut uploaded = vec![into_the_loom, her_own];
    uploaded.sort();
    assert_eq!(listed, uploaded);
    assert!(hers.listed.iter().all(|own| own.deletable));
    assert_eq!(hers.next, None);
    // and: nothing of Bo's, who uploaded nothing, though he reads the loom
    assert!(his.listed.is_empty());
}

#[tokio::test]
async fn an_upload_into_a_project_one_may_no_longer_write_in_may_not_be_deleted() {
    // given: Ada's uploads, Ada reading the loom and no more
    let (bench, into_the_loom, her_own) = ada_having_uploaded().await;
    ada_only_reads_the_loom(&bench).await;

    // when: she lists them
    let hers = bench
        .assets
        .uploads_of(UploadsSorted::Largest, None, 10, &bench::ada())
        .await
        .expect("an account is told its uploads");

    // then: the loom's is not hers to delete, her own still is
    let deletable = |id: AssetId| {
        hers.listed
            .iter()
            .find(|own| own.upload.id == id)
            .map(|own| own.deletable)
    };
    assert_eq!(deletable(into_the_loom), Some(false));
    assert_eq!(deletable(her_own), Some(true));
    // and: deleting it is refused as told
    let refused = bench
        .assets
        .delete(&into_the_loom.to_string(), &bench::ada())
        .await;
    assert!(
        matches!(refused, Err(AssetsError::NotAnAuthor)),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_full_page_of_uploads_goes_on_from_its_last() {
    // given: Ada's two uploads
    let (bench, ..) = ada_having_uploaded().await;

    // when: she lists them a page of one at a time
    let first = bench
        .assets
        .uploads_of(UploadsSorted::Largest, None, 1, &bench::ada())
        .await
        .expect("an account is told its uploads");
    let then = bench
        .assets
        .uploads_of(
            UploadsSorted::Largest,
            first.next.as_ref(),
            1,
            &bench::ada(),
        )
        .await
        .expect("an account is told its uploads");

    // then: each page holds another, and the second goes on to nothing more
    assert_eq!((first.listed.len(), then.listed.len()), (1, 1));
    assert_ne!(first.listed[0].upload.id, then.listed[0].upload.id);
    let last = bench
        .assets
        .uploads_of(UploadsSorted::Largest, then.next.as_ref(), 1, &bench::ada())
        .await
        .expect("an account is told its uploads");
    assert!(last.listed.is_empty());
}

#[tokio::test]
async fn without_accounts_nobody_has_uploads_of_their_own() {
    // given: the instance without accounts
    let bench = bench::a_bench_without_accounts(Default::default()).await;

    // when: the one working asks for their uploads
    let refused = bench
        .assets
        .uploads_of(UploadsSorted::Newest, None, 10, &Agent::Local)
        .await;

    // then: it is refused
    assert!(
        matches!(refused, Err(AssetsError::NoAccount)),
        "{refused:?}"
    );
}

#[tokio::test]
async fn the_route_lists_an_account_s_uploads_and_goes_on_from_its_cursor() {
    // given: Ada's uploads, Ada reading the loom and no more, the route for her
    let (bench, into_the_loom, _) = ada_having_uploaded().await;
    ada_only_reads_the_loom(&bench).await;
    let hers = serving(&bench, bench::ada());

    // when: she lists them, the largest first
    let listed: UploadsDTO = hers.get("/assets?sort=largest").await.json();

    // then: both, the loom's by its name, told when and whether she may delete it
    assert_eq!(listed.uploads.len(), 2);
    assert_eq!(listed.next, None, "a page holds fifty");
    let loom = listed
        .uploads
        .iter()
        .find(|upload| upload.id == into_the_loom.to_string())
        .expect("the loom's upload is listed");
    assert_eq!(loom.owner, OwnerDTO::Project(THE_LOOM.to_owned()));
    assert_eq!(loom.project_name.as_deref(), Some("Loom"));
    assert_eq!(loom.format, "jpeg");
    assert_eq!(loom.uploaded_at, "2023-11-14T22:13:20Z");
    assert!(!loom.deletable);
    // when: she goes on from the first
    let first = &listed.uploads[0];
    let cursor = format!(
        "{}.{}.{}",
        first.total_size,
        1_700_000_000_i128 * 1_000_000_000,
        first.id
    );
    let after: UploadsDTO = hers
        .get(&format!("/assets?sort=largest&after={cursor}"))
        .await
        .json();
    // then: the second alone
    assert_eq!(after.uploads.len(), 1);
    assert_eq!(after.uploads[0].id, listed.uploads[1].id);
}

#[tokio::test]
async fn the_route_refuses_an_unknown_sort_a_cursor_that_is_none_and_nobody_signed_in() {
    // given: Ada's uploads, the routes for her and for the instance without accounts
    let (bench, ..) = ada_having_uploaded().await;
    let hers = serving(&bench, bench::ada());
    let nobodys = serving(&bench, Agent::Local);

    // expect: an unknown sort refused
    let unknown = hers.get("/assets?sort=oldest").await;
    unknown.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(unknown.json::<Value>()["type"], refused::SORT_UNKNOWN);
    // and: a cursor that is none
    for none in ["12", "12.34", "12.34.asset_nothing", "twelve.34.asset_x"] {
        let refused_cursor = hers.get(&format!("/assets?after={none}")).await;
        refused_cursor.assert_status(StatusCode::BAD_REQUEST);
        assert_eq!(
            refused_cursor.json::<Value>()["type"],
            refused::CURSOR_INVALID,
            "{none}"
        );
    }
    // and: whoever is not signed in
    let by_nobody = nobodys.get("/assets").await;
    by_nobody.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(by_nobody.json::<Value>()["type"], refused::NO_ACCOUNT);
}
