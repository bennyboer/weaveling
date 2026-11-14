use accounts_contract::{PICTURE_CHANGED, PICTURE_REMOVED};
use assets_contract::{AssetDTO, AssetUseDTO, HolderDTO, purpose, refused};
use assets_core::{AssetId, AssetUse, AssetsError, Holder, Purpose, UploadsSorted};
use assets_messaging::RememberPicturesListener;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use eventsourcing::{AccountId, Agent};
use messaging::{Listener, Message, RoutingKey};
use serde_json::{Value, json};

use crate::bench::{self, Bench, THE_LOOM};
use crate::names_heard::{ada_deleted, inboxed, people};

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";
const ADA: &str = "account_ada";

fn told_of_ada(routing: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "account",
        ADA,
        version,
        event,
        &Agent::Account(AccountId::from(ADA)),
        clock::at(1_700_000_000),
    )
}

fn changed(version: u64, picture: &AssetId, previous: Option<&AssetId>, drop: bool) -> Message {
    told_of_ada(
        PICTURE_CHANGED,
        version,
        json!({
            "version": version,
            "name": "PICTURE_CHANGED",
            "picture": picture.to_string(),
            "previous": previous.map(ToString::to_string),
            "drop_previous": drop,
        }),
    )
}

fn removed(version: u64, picture: &AssetId, drop: bool) -> Message {
    told_of_ada(
        PICTURE_REMOVED,
        version,
        json!({
            "version": version,
            "name": "PICTURE_REMOVED",
            "picture": picture.to_string(),
            "drop": drop,
        }),
    )
}

async fn heard(bench: &Bench, message: Message) {
    inboxed(RememberPicturesListener::new(bench.assets.clone()))
        .handle(&message)
        .await
        .expect("a change of a picture is taken in");
}

async fn adas_own(bench: &Bench) -> AssetId {
    bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an account uploads its own")
        .0
}

/// What the asset is used for, as Ada is told; none if she cannot see it any more.
async fn used_as(bench: &Bench, id: &AssetId) -> Option<Vec<AssetUse>> {
    bench
        .assets
        .asset(&id.to_string(), &bench::ada())
        .await
        .ok()
        .map(|(_, uses)| uses)
}

fn ada_s_picture() -> AssetUse {
    AssetUse {
        purpose: Purpose::AccountPicture,
        by: Holder::Account(AccountId::from(ADA)),
    }
}

#[tokio::test]
async fn the_listener_hears_a_picture_changed_and_removed() {
    // given: the listener
    let bench = bench::a_bench().await;
    let listener = inboxed(RememberPicturesListener::new(bench.assets.clone()));

    // expect: it hears both
    for routing in [PICTURE_CHANGED, PICTURE_REMOVED] {
        assert!(
            listener.hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn a_picture_in_use_is_not_deleted_and_the_list_says_why() {
    // given: Ada's own image shown as her picture
    let bench = bench::a_bench().await;
    let picture = adas_own(&bench).await;
    heard(&bench, changed(2, &picture, None, false)).await;

    // then: it is used as her picture
    assert_eq!(used_as(&bench, &picture).await, Some(vec![ada_s_picture()]));
    // and: deleting it is refused
    let refused = bench
        .assets
        .delete(&picture.to_string(), &bench::ada())
        .await;
    assert!(matches!(refused, Err(AssetsError::InUse)), "{refused:?}");
    // and: her uploads say it is in use and not deletable
    let listed = bench
        .assets
        .uploads_of(UploadsSorted::Newest, None, 10, &bench::ada())
        .await
        .expect("an account is told its uploads");
    assert!(!listed.listed[0].deletable);
    assert_eq!(listed.listed[0].uses, vec![ada_s_picture()]);
}

#[tokio::test]
async fn the_routes_tell_an_asset_s_uses_and_refuse_deleting_one_in_use() {
    // given: Ada's own image shown as her picture, the routes for her
    let bench = bench::a_bench().await;
    let picture = adas_own(&bench).await;
    heard(&bench, changed(2, &picture, None, false)).await;
    let hers =
        TestServer::new(assets_rest::router(bench.assets.clone()).layer(Extension(bench::ada())));

    // when: she asks for the asset
    let told: AssetDTO = hers.get(&format!("/assets/{picture}")).await.json();

    // then: it says it is her picture
    assert_eq!(
        told.uses,
        vec![AssetUseDTO {
            purpose: purpose::ACCOUNT_PICTURE.to_owned(),
            by: HolderDTO::Account(ADA.to_owned()),
        }]
    );
    // and: deleting it is refused as in use
    let refused_delete = hers.delete(&format!("/assets/{picture}")).await;
    refused_delete.assert_status(StatusCode::CONFLICT);
    assert_eq!(refused_delete.json::<Value>()["type"], refused::IN_USE);
}

#[tokio::test]
async fn a_picture_replaced_is_deleted_when_asked_and_otherwise_kept_free() {
    // given: three images of Ada's own, the first shown as her picture
    let bench = bench::a_bench().await;
    let (first, second, third) = (
        adas_own(&bench).await,
        adas_own(&bench).await,
        adas_own(&bench).await,
    );
    heard(&bench, changed(2, &first, None, false)).await;

    // when: the second replaces it, the first to be kept
    heard(&bench, changed(3, &second, Some(&first), false)).await;
    // then: the first is kept, and no longer in use
    assert_eq!(used_as(&bench, &first).await, Some(vec![]));
    // when: the third replaces the second, which is to be deleted
    heard(&bench, changed(4, &third, Some(&second), true)).await;
    // then: the second is gone
    assert_eq!(used_as(&bench, &second).await, None);
    // and: the third is her picture
    assert_eq!(used_as(&bench, &third).await, Some(vec![ada_s_picture()]));
}

#[tokio::test]
async fn a_picture_removed_is_deleted_when_asked_and_otherwise_kept_free() {
    // given: two images of Ada's own
    let bench = bench::a_bench().await;
    let (kept, dropped) = (adas_own(&bench).await, adas_own(&bench).await);

    // when: one is shown and then removed, to be kept
    heard(&bench, changed(2, &kept, None, false)).await;
    heard(&bench, removed(3, &kept, false)).await;
    // then: it is kept, and no longer in use
    assert_eq!(used_as(&bench, &kept).await, Some(vec![]));
    // when: the other is shown and then removed, to be deleted
    heard(&bench, changed(4, &dropped, None, false)).await;
    heard(&bench, removed(5, &dropped, true)).await;
    // then: it is gone
    assert_eq!(used_as(&bench, &dropped).await, None);
}

#[tokio::test]
async fn only_an_image_of_the_account_s_own_counts_as_its_picture_or_is_deleted_with_it() {
    // given: a photo in the loom, Bo's own portrait, and Ada's own image as her picture
    let bench = bench::a_bench().await;
    let in_the_loom = bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an author uploads")
        .0;
    let bos = bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &bench::bo())
        .await
        .expect("an account uploads its own")
        .0;
    let hers = adas_own(&bench).await;

    // when: the loom's photo is told to be her picture
    heard(&bench, changed(2, &in_the_loom, None, false)).await;
    // then: it is not in use
    assert_eq!(used_as(&bench, &in_the_loom).await, Some(vec![]));
    // when: Bo's portrait is told to be her picture, and the loom's photo to be deleted
    heard(&bench, changed(3, &bos, Some(&in_the_loom), true)).await;
    // then: neither is used as her picture, and the loom keeps its photo
    let bo_s_uses = bench
        .assets
        .asset(&bos.to_string(), &bench::bo())
        .await
        .expect("Bo sees his own")
        .1;
    assert!(bo_s_uses.is_empty());
    assert_eq!(used_as(&bench, &in_the_loom).await, Some(vec![]));
    // when: her own image is told to be her picture
    heard(&bench, changed(4, &hers, None, false)).await;
    // then: that one is in use
    assert_eq!(used_as(&bench, &hers).await, Some(vec![ada_s_picture()]));
}

#[tokio::test]
async fn a_deleted_account_s_picture_is_in_use_no_more_and_its_tombstone_is_let_go() {
    // given: Ada's own image shown as her picture
    let bench = bench::a_bench().await;
    let picture = adas_own(&bench).await;
    heard(&bench, changed(2, &picture, None, false)).await;

    // when: her account's deletion is heard
    inboxed(people(&bench))
        .handle(&ada_deleted(6))
        .await
        .expect("a deletion is taken in");

    // then: her picture is in use no more
    let uses = bench
        .assets
        .asset(&picture.to_string(), &bench::ada())
        .await
        .expect("her own image is still there until swept")
        .1;
    assert!(uses.is_empty());
    // and: the tombstones of her name and her picture are let go of later
    assert_eq!(
        bench
            .assets
            .forget_tombstones_laid_before(clock::at(1_700_000_001), 100)
            .await
            .expect("forgetting should succeed"),
        2
    );
}
