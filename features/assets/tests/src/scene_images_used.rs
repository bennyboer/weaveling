use access::{HearsAccess, Role};
use assets_contract::{AssetDTO, AssetUseDTO, HolderDTO, purpose};
use assets_core::{AssetId, AssetUse, AssetsError, Holder, Purpose, SceneLink};
use assets_messaging::RememberSceneImagesListener;
use axum::Extension;
use axum_test::TestServer;
use eventsourcing::{AccountId, Version};
use messaging::{Listener, Message, RoutingKey};
use scenes_contract::{ASSETS_CHANGED, DELETED};
use serde_json::json;

use crate::bench::{self, Bench, THE_LOOM, THE_WEFT};
use crate::names_heard::inboxed;

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";
const DAWN: &str = "scene_dawn";

fn images_of_dawn(project: &str, images: &[&AssetId], version: u64) -> Message {
    Message::opening(
        RoutingKey::parse(ASSETS_CHANGED).expect("a declared key"),
        json!({
            "scene": DAWN,
            "assets": images.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "version": version,
            "context": { "project": project },
        }),
        clock::at(1_700_000_000),
    )
}

fn dawn_deleted() -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared key"),
        json!({ "scene": DAWN, "context": { "project": THE_LOOM } }),
        clock::at(1_700_000_000),
    )
}

async fn heard(bench: &Bench, message: Message) {
    inboxed(RememberSceneImagesListener::new(bench.assets.clone()))
        .handle(&message)
        .await
        .expect("what a scene holds is taken in");
}

async fn a_photo_in(bench: &Bench, project: &str) -> AssetId {
    bench
        .assets
        .upload_into(project, A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an author uploads")
        .0
}

/// What the asset is used for, as Ada is told.
async fn used_as(bench: &Bench, id: &AssetId) -> Vec<AssetUse> {
    bench
        .assets
        .asset(&id.to_string(), &bench::ada())
        .await
        .expect("Ada sees the asset")
        .1
}

fn in_dawn() -> AssetUse {
    AssetUse {
        purpose: Purpose::SceneImage,
        by: Holder::Scene(SceneLink::from(DAWN)),
    }
}

#[tokio::test]
async fn the_listener_hears_the_images_of_a_scene_and_its_deletion() {
    // given: the listener
    let bench = bench::a_bench().await;
    let listener = inboxed(RememberSceneImagesListener::new(bench.assets.clone()));

    // expect: it hears both
    for routing in [ASSETS_CHANGED, DELETED] {
        assert!(
            listener.hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn an_image_a_scene_holds_cannot_be_deleted_until_the_scene_lets_go_of_it() {
    // given: a photo in the loom
    let bench = bench::a_bench().await;
    let photo = a_photo_in(&bench, THE_LOOM).await;

    // when: a scene of the loom is told to hold it
    heard(&bench, images_of_dawn(THE_LOOM, &[&photo], 2)).await;
    // then: it is in use by the scene
    assert_eq!(used_as(&bench, &photo).await, vec![in_dawn()]);
    // and: deleting it is refused
    let refused = bench.assets.delete(&photo.to_string(), &bench::ada()).await;
    assert!(matches!(refused, Err(AssetsError::InUse)), "{refused:?}");

    // when: the scene is told to hold no images any more
    heard(&bench, images_of_dawn(THE_LOOM, &[], 3)).await;
    // then: it is in use no more
    assert!(used_as(&bench, &photo).await.is_empty());
    // and: it can be deleted
    bench
        .assets
        .delete(&photo.to_string(), &bench::ada())
        .await
        .expect("an image no scene holds is deleted");
}

#[tokio::test]
async fn the_route_tells_that_a_scene_holds_an_image() {
    // given: a photo in the loom held by a scene, the routes for Ada
    let bench = bench::a_bench().await;
    let photo = a_photo_in(&bench, THE_LOOM).await;
    heard(&bench, images_of_dawn(THE_LOOM, &[&photo], 2)).await;
    let hers =
        TestServer::new(assets_rest::router(bench.assets.clone()).layer(Extension(bench::ada())));

    // when: she asks for the asset
    let told: AssetDTO = hers.get(&format!("/assets/{photo}")).await.json();

    // then: it says the scene holds it
    assert_eq!(
        told.uses,
        vec![AssetUseDTO {
            purpose: purpose::SCENE_IMAGE.to_owned(),
            by: HolderDTO::Scene(DAWN.to_owned()),
        }]
    );
}

#[tokio::test]
async fn a_scene_keeps_only_images_of_its_own_project() {
    // given: Ada also runs the weft
    let bench = bench::a_bench().await;
    bench
        .assets
        .remember_role(
            None,
            THE_WEFT,
            &AccountId::from("account_ada"),
            Version::of(3),
            Some(Role::Owner),
        )
        .await
        .expect("remembering should succeed");
    // and: a photo in the loom, one in the weft, and one of her own
    let (in_the_loom, in_the_weft) = (
        a_photo_in(&bench, THE_LOOM).await,
        a_photo_in(&bench, THE_WEFT).await,
    );
    let hers = bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an account uploads its own")
        .0;

    // when: a scene of the loom is told to hold all three
    heard(
        &bench,
        images_of_dawn(THE_LOOM, &[&in_the_loom, &in_the_weft, &hers], 2),
    )
    .await;

    // then: only the loom's photo is in use
    assert_eq!(used_as(&bench, &in_the_loom).await, vec![in_dawn()]);
    assert!(
        used_as(&bench, &in_the_weft).await.is_empty(),
        "a scene may not keep another project's image from being deleted"
    );
    assert!(
        used_as(&bench, &hers).await.is_empty(),
        "a scene may not keep an account's own image from being deleted"
    );
}

#[tokio::test]
async fn a_deleted_scene_lets_go_of_its_images_for_good() {
    // given: a photo in the loom held by a scene
    let bench = bench::a_bench().await;
    let photo = a_photo_in(&bench, THE_LOOM).await;
    heard(&bench, images_of_dawn(THE_LOOM, &[&photo], 2)).await;

    // when: the scene's deletion is heard
    heard(&bench, dawn_deleted()).await;
    // then: the photo is in use no more
    assert!(used_as(&bench, &photo).await.is_empty());

    // when: a later count of the scene arrives late
    heard(&bench, images_of_dawn(THE_LOOM, &[&photo], 9)).await;
    // then: the photo stays free
    assert!(
        used_as(&bench, &photo).await.is_empty(),
        "a deleted scene holds nothing, whatever arrives about it later"
    );
    // and: it can be deleted
    bench
        .assets
        .delete(&photo.to_string(), &bench::ada())
        .await
        .expect("an image no scene holds is deleted");
}

#[tokio::test]
async fn a_deleted_scene_s_tombstone_is_let_go_of_after_its_time() {
    // given: a photo in the loom held by a deleted scene
    let bench = bench::a_bench().await;
    let photo = a_photo_in(&bench, THE_LOOM).await;
    heard(&bench, images_of_dawn(THE_LOOM, &[&photo], 2)).await;
    heard(&bench, dawn_deleted()).await;

    // expect: its tombstone is let go of after its time
    assert_eq!(
        bench
            .assets
            .forget_tombstones_laid_before(clock::at(1_700_000_001), 100)
            .await
            .expect("forgetting should succeed"),
        1
    );
}
