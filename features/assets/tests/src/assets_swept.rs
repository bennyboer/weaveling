use assets_core::{AssetId, Contents, Owner, Piece, ProjectLink, SWEPT_AT_ONCE};
use eventsourcing::AccountId;

use crate::bench::{self, Bench, THE_LOOM};

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";

async fn into_the_loom(bench: &Bench) -> AssetId {
    bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an author uploads")
        .0
}

async fn adas_own(bench: &Bench) -> AssetId {
    bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an account uploads its own")
        .0
}

/// Whether the asset is gone: it cannot be fetched, and its original bytes are no longer stored.
async fn gone(bench: &Bench, id: &AssetId) -> bool {
    bench
        .assets
        .content(&id.to_string(), None, &bench::ada())
        .await
        .is_err()
        && bench
            .contents
            .read(id, Piece::Original)
            .await
            .expect("the contents should be reached")
            .is_none()
}

fn the_loom() -> Owner {
    Owner::Project(ProjectLink::from(THE_LOOM))
}

fn ada() -> Owner {
    Owner::Account(AccountId::from("account_ada"))
}

#[tokio::test]
async fn a_project_swept_loses_every_asset_and_its_pieces_and_nothing_else() {
    // given: two photos in the loom and a portrait of Ada's own
    let bench = bench::a_bench().await;
    let photos = [into_the_loom(&bench).await, into_the_loom(&bench).await];
    let portrait = adas_own(&bench).await;

    // when: the loom is swept
    let swept = bench.assets.sweep(&the_loom()).await.expect("sweeping");

    // then: both photos are gone, pieces and all
    assert_eq!(swept, 2);
    for photo in &photos {
        assert!(gone(&bench, photo).await);
    }
    // and: the loom holds nothing any more, Ada her portrait alone
    let usage = bench.usage().await;
    assert_eq!(usage.project, 0);
    assert!(usage.account > 0);
    // and: her portrait is still there
    assert!(!gone(&bench, &portrait).await);
}

#[tokio::test]
async fn an_account_swept_loses_its_own_assets_but_not_what_it_uploaded_into_projects() {
    // given: a photo Ada put in the loom and a portrait of her own
    let bench = bench::a_bench().await;
    let photo = into_the_loom(&bench).await;
    let portrait = adas_own(&bench).await;

    // when: Ada's own are swept
    let swept = bench.assets.sweep(&ada()).await.expect("sweeping");

    // then: her portrait is gone
    assert_eq!(swept, 1);
    assert!(gone(&bench, &portrait).await);
    // and: the loom keeps her photo
    assert!(!gone(&bench, &photo).await);
}

#[tokio::test]
async fn more_than_a_batch_is_swept_batch_after_batch() {
    // given: a batch of photos in the loom and one more
    let bench = bench::a_bench().await;
    let mut photos = Vec::new();
    for _ in 0..=SWEPT_AT_ONCE {
        photos.push(into_the_loom(&bench).await);
    }

    // when: the loom is swept
    let swept = bench.assets.sweep(&the_loom()).await.expect("sweeping");

    // then: every one is gone
    assert_eq!(swept, SWEPT_AT_ONCE + 1);
    for photo in &photos {
        assert!(gone(&bench, photo).await);
    }
}

#[tokio::test]
async fn sweeping_what_holds_nothing_changes_nothing() {
    // given: a photo in the loom, swept once
    let bench = bench::a_bench().await;
    into_the_loom(&bench).await;
    bench.assets.sweep(&the_loom()).await.expect("sweeping");

    // expect: sweeping it again, or an owner that never held anything, sweeps nothing
    assert_eq!(bench.assets.sweep(&the_loom()).await.expect("sweeping"), 0);
    assert_eq!(bench.assets.sweep(&ada()).await.expect("sweeping"), 0);
}
