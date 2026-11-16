use assets_core::{AssetId, Contents, Piece};

use crate::wiring::{self, THE_LOOM, Wired};

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";

/// The wired's clock reads this moment.
const NOW: i64 = 1_700_000_000;

/// Stores an original for an asset whose id was made at the given moment, without recording the asset.
/// This is what a crash between deleting an asset and removing its bytes leaves behind.
async fn a_stray_made_at(wired: &Wired, seconds: i64) -> AssetId {
    let stray = AssetId::generate(clock::at(seconds));
    wired
        .contents
        .keep(&stray, Piece::Original, b"left behind")
        .await
        .expect("keeping should succeed");

    stray
}

async fn stored(wired: &Wired, id: &AssetId) -> bool {
    wired
        .contents
        .read(id, Piece::Original)
        .await
        .expect("reading should succeed")
        .is_some()
}

#[tokio::test]
async fn old_pieces_of_no_asset_are_removed_and_everything_else_stays() {
    // given: a photo uploaded long ago, a stray from two hours ago, and a stray from a minute ago
    let wired = wiring::a_workbench().await;
    let (photo, _) = wired
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &wiring::ada())
        .await
        .expect("an author uploads");
    let old_stray = a_stray_made_at(&wired, NOW - 2 * 60 * 60).await;
    let young_stray = a_stray_made_at(&wired, NOW - 60).await;

    // when: the strays made over an hour ago are removed, as if checked a day later
    let removed = wired
        .assets
        .forget_stray_contents(clock::at(NOW - 60 * 60))
        .await
        .expect("removing strays should succeed");

    // then: only the old stray is gone
    assert_eq!(removed, 1);
    assert!(!stored(&wired, &old_stray).await);
    // and: the young stray stays, as it may be an upload still in progress
    assert!(stored(&wired, &young_stray).await);
    // and: the recorded photo, made at the wired's moment, stays too when checked later
    wired
        .assets
        .forget_stray_contents(clock::at(NOW + 24 * 60 * 60))
        .await
        .expect("removing strays should succeed");
    assert!(stored(&wired, &photo).await);
}

#[tokio::test]
async fn strays_beyond_one_check_are_all_removed() {
    // given: more strays than are checked at once
    let wired = wiring::a_workbench().await;
    let mut strays = Vec::new();
    for seconds in 0..201 {
        strays.push(a_stray_made_at(&wired, NOW - 10_000 + seconds).await);
    }

    // when: the old strays are removed
    let removed = wired
        .assets
        .forget_stray_contents(clock::at(NOW))
        .await
        .expect("removing strays should succeed");

    // then: every one is gone
    assert_eq!(removed, 201);
    assert_eq!(wired.contents.pieces(), 0);
}
