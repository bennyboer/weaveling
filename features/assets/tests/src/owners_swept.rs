use assets_contract::OwnerDTO;
use assets_core::AssetId;
use assets_messaging::{SWEEP_OWNER, SweepAssetsOfDeletedOwnerListener, SweepOwnerDTO};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::RENAMED;

use crate::bench::{self, Bench, THE_LOOM};
use crate::names_heard::{
    ada_deleted, ada_signed_up, inboxed, people, project_names, the_loom_deleted,
};

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";

/// Ada uploads a photo into the loom and a portrait of her own.
async fn ada_having_uploaded() -> (Bench, AssetId, AssetId) {
    let bench = bench::a_bench().await;
    let (photo, _) = bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an author uploads");
    let (portrait, _) = bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &bench::ada())
        .await
        .expect("an account uploads its own");

    (bench, photo, portrait)
}

/// Whether Ada can still fetch the asset.
async fn still_there(bench: &Bench, id: &AssetId) -> bool {
    bench
        .assets
        .content(&id.to_string(), None, &bench::ada())
        .await
        .is_ok()
}

/// Hands every follow-up queued so far to the sweep listener.
async fn swept(bench: &Bench, follow_ups: &[Message]) {
    let sweeping = SweepAssetsOfDeletedOwnerListener::new(bench.assets.clone());
    for follow_up in follow_ups {
        sweeping
            .handle(follow_up)
            .await
            .expect("a sweep is taken in");
    }
}

fn swept_owner(follow_up: &Message) -> OwnerDTO {
    follow_up
        .payload_as::<SweepOwnerDTO>()
        .expect("a sweep is shaped as declared")
        .owner
}

#[tokio::test]
async fn the_sweep_listener_hears_the_sweep_and_not_the_deletions() {
    // given: the sweep listener
    let bench = bench::a_bench().await;
    let sweeping = SweepAssetsOfDeletedOwnerListener::new(bench.assets.clone());

    // expect: it hears a sweep
    assert!(sweeping.hears(&RoutingKey::parse(SWEEP_OWNER).expect("a declared key")));
    // and: not the deletions themselves
    for deleted in [accounts_contract::DELETED, projects_contract::DELETED] {
        assert!(
            !sweeping.hears(&RoutingKey::parse(deleted).expect("a declared key")),
            "the sweep waits until the deletion was heard and committed"
        );
    }
}

#[tokio::test]
async fn a_deleted_project_s_assets_are_swept() {
    // given: Ada's photo in the loom and her portrait
    let (bench, photo, portrait) = ada_having_uploaded().await;

    // when: the loom's deletion is heard
    inboxed(project_names(&bench))
        .handle(&the_loom_deleted(4))
        .await
        .expect("a deletion is taken in");

    // then: the loom's sweep is asked for, once
    let follow_ups = bench.follow_ups().await;
    assert_eq!(follow_ups.len(), 1);
    assert_eq!(follow_ups[0].routing.to_string(), SWEEP_OWNER);
    assert_eq!(
        swept_owner(&follow_ups[0]),
        OwnerDTO::Project(THE_LOOM.to_owned())
    );
    // when: the sweep runs
    swept(&bench, &follow_ups).await;
    // then: the photo is gone, and the portrait is still there
    assert!(!still_there(&bench, &photo).await);
    assert!(still_there(&bench, &portrait).await);
}

#[tokio::test]
async fn a_deleted_account_s_own_assets_are_swept_but_not_its_uploads_into_projects() {
    // given: Ada's photo in the loom and her portrait
    let (bench, photo, portrait) = ada_having_uploaded().await;

    // when: Ada's deletion is heard
    inboxed(people(&bench))
        .handle(&ada_deleted(5))
        .await
        .expect("a deletion is taken in");

    // then: the sweep of her own assets is asked for
    let follow_ups = bench.follow_ups().await;
    assert_eq!(follow_ups.len(), 1);
    assert_eq!(
        swept_owner(&follow_ups[0]),
        OwnerDTO::Account("account_ada".to_owned())
    );
    // when: the sweep runs
    swept(&bench, &follow_ups).await;
    // then: her portrait is gone, and the loom keeps her photo
    assert!(!still_there(&bench, &portrait).await);
    assert!(still_there(&bench, &photo).await);
}

#[tokio::test]
async fn a_sign_up_or_a_rename_asks_for_no_sweep() {
    // given: Ada's uploads
    let (bench, ..) = ada_having_uploaded().await;

    // when: her sign-up and a renaming of the loom are heard
    inboxed(people(&bench))
        .handle(&ada_signed_up(1, "Ada Lovelace", "ada@example.com"))
        .await
        .expect("a sign-up is taken in");
    inboxed(project_names(&bench))
        .handle(&crate::names_heard::the_loom_named(RENAMED, 2, "Loom"))
        .await
        .expect("a renaming is taken in");

    // then: no sweep is asked for
    assert!(bench.follow_ups().await.is_empty());
}
