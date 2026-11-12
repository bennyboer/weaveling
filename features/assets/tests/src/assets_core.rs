use assets_core::{
    AssetsError, Bound, Format, ImageRefused, Limit, Limits, Owner, ProjectLink, Usage,
};
use eventsourcing::{AccountId, Agent, ServiceError};

use crate::bench::{THE_LOOM, a_bench, a_bench_limited, a_bench_refusing, ada, bo, grace};

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";

fn is_not_found<T: std::fmt::Debug>(result: &Result<T, AssetsError>) -> bool {
    matches!(
        result,
        Err(AssetsError::Events(ServiceError::NotFound { .. }))
    )
}

#[tokio::test]
async fn an_author_uploads_a_photo_into_a_project_kept_with_its_variants_and_what_they_take() {
    // given: Ada, an owner of the loom
    let bench = a_bench();

    // when: she uploads a photo into it
    let (id, standing) = bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &ada())
        .await
        .expect("an author uploads");

    // then: it is the loom's, the photo as it was prepared
    let asset = standing.state;
    assert_eq!(asset.owner(), &Owner::Project(ProjectLink::from(THE_LOOM)));
    assert_eq!(asset.picture().format, Format::Jpeg);
    assert_eq!(asset.picture().original_size, A_PHOTO.len() as u64);
    assert_eq!(asset.picture().variants, [384, 768, 1_280]);
    // and: what it takes is the original and every variant
    let variants: u64 = [384, 768, 1_280]
        .map(|width| format!("the {width} wide variant").len() as u64)
        .iter()
        .sum();
    assert_eq!(asset.total_size(), A_PHOTO.len() as u64 + variants);
    // and: the original and three variants are kept
    assert_eq!(bench.contents.pieces(), 4);
    // and: the installation, Ada and the loom hold it
    let total = asset.total_size();
    assert_eq!(
        bench.usage().await,
        Usage {
            instance: total,
            account: total,
            project: total,
        }
    );
    // and: it was asked of the images once
    assert_eq!(bench.images.asked(), 1);
    assert!(!id.to_string().is_empty());
}

#[tokio::test]
async fn whoever_reads_the_project_sees_the_narrowest_variant_as_wide_as_asked_and_never_the_original()
 {
    // given: a photo of 1,280 pixels in the loom, made into 384, 768 and 1,280
    let bench = a_bench();
    let (id, _) = bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &ada())
        .await
        .expect("an author uploads");
    let id = id.to_string();

    // expect: Bo, a reader, sees for each width asked the narrowest variant at least as wide, else the widest
    for (asked, given) in [
        (Some(1), 384),
        (Some(384), 384),
        (Some(500), 768),
        (Some(1_280), 1_280),
        (Some(5_000), 1_280),
        (None, 1_280),
    ] {
        let content = bench
            .assets
            .content(&id, asked, &bo())
            .await
            .expect("a reader sees it");
        assert_eq!(
            content.bytes,
            format!("the {given} wide variant").into_bytes(),
            "asked for {asked:?}"
        );
        assert_eq!(content.content_type, "image/webp");
    }
}

#[tokio::test]
async fn whoever_may_not_read_the_project_finds_no_asset_nor_project_and_a_reader_changes_nothing()
{
    // given: a photo in the loom
    let bench = a_bench();
    let (id, _) = bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &ada())
        .await
        .expect("an author uploads");
    let id = id.to_string();

    // expect: Grace, no member, finds no such asset, nor project to upload into
    assert!(is_not_found(
        &bench.assets.content(&id, None, &grace()).await
    ));
    assert!(is_not_found(&bench.assets.delete(&id, &grace()).await));
    assert!(matches!(
        bench
            .assets
            .upload_into(THE_LOOM, A_PHOTO.to_vec(), &grace())
            .await,
        Err(AssetsError::ProjectNotThere)
    ));
    // and: Bo, a reader, neither uploads nor deletes
    assert!(matches!(
        bench
            .assets
            .upload_into(THE_LOOM, A_PHOTO.to_vec(), &bo())
            .await,
        Err(AssetsError::NotAnAuthor)
    ));
    assert!(matches!(
        bench.assets.delete(&id, &bo()).await,
        Err(AssetsError::NotAnAuthor)
    ));
    // and: nothing was asked of the images for those refused
    assert_eq!(bench.images.asked(), 1);
}

#[tokio::test]
async fn an_upload_past_its_own_limit_is_refused_before_anything_is_made_of_it() {
    // given: uploads of ten bytes at most
    let bench = a_bench_limited(Limits {
        each_upload: Some(10),
        ..Limits::default()
    });

    // when: eleven bytes are uploaded
    let refused = bench
        .assets
        .upload_into(THE_LOOM, vec![0; 11], &ada())
        .await;

    // then: it is refused by the upload's limit
    assert!(
        matches!(
            refused,
            Err(AssetsError::LimitReached(Limit {
                bound: Bound::Upload,
                most: 10,
                used: 0,
            }))
        ),
        "{refused:?}"
    );
    // and: nothing was made of it, nor kept
    assert_eq!(
        bench.images.asked(),
        0,
        "an upload too large is not read at all"
    );
    assert_eq!(bench.contents.pieces(), 0);
}

#[tokio::test]
async fn an_upload_past_the_accounts_or_the_projects_or_the_installations_limit_is_refused_and_keeps_nothing()
 {
    // given: what one photo takes
    let one = {
        let bench = a_bench();
        bench
            .assets
            .upload_into(THE_LOOM, A_PHOTO.to_vec(), &ada())
            .await
            .expect("an author uploads")
            .1
            .state
            .total_size()
    };

    for (limits, bound) in [
        (
            Limits {
                each_account: Some(one * 3 / 2),
                ..Limits::default()
            },
            Bound::Account,
        ),
        (
            Limits {
                each_project: Some(one * 3 / 2),
                ..Limits::default()
            },
            Bound::Project,
        ),
        (
            Limits {
                instance: Some(one * 3 / 2),
                ..Limits::default()
            },
            Bound::Instance,
        ),
    ] {
        // given: room for one photo and a half under one limit, one photo uploaded
        let bench = a_bench_limited(limits);
        bench
            .assets
            .upload_into(THE_LOOM, A_PHOTO.to_vec(), &ada())
            .await
            .expect("the first fits");

        // when: a second is uploaded
        let refused = bench
            .assets
            .upload_into(THE_LOOM, A_PHOTO.to_vec(), &ada())
            .await;

        // then: it is refused, told which limit, its most and what it holds
        assert!(
            matches!(
                refused,
                Err(AssetsError::LimitReached(Limit { bound: told, most, used }))
                    if told == bound && most == one * 3 / 2 && used == one
            ),
            "{bound:?}: {refused:?}"
        );
        // and: nothing of it is kept, the first alone held
        assert_eq!(bench.contents.pieces(), 4, "{bound:?}");
        assert_eq!(bench.usage().await.instance, one, "{bound:?}");
    }
}

#[tokio::test]
async fn without_accounts_nobody_is_held_to_an_accounts_limit() {
    // given: an account's limit of one byte, and nobody signed in, as in local mode
    let bench = a_bench_limited(Limits {
        each_account: Some(1),
        ..Limits::default()
    });

    // expect: an upload is taken
    bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &Agent::Local)
        .await
        .expect("an account's limit holds only accounts");
}

#[tokio::test]
async fn an_upload_that_is_no_image_is_refused_as_such_and_kept_nowhere() {
    // given: images that take nothing
    let bench = a_bench_refusing(ImageRefused::NotAnImage);

    // when: something is uploaded
    let refused = bench
        .assets
        .upload_into(THE_LOOM, b"<svg/>".to_vec(), &ada())
        .await;

    // then: it is refused as no image
    assert!(
        matches!(refused, Err(AssetsError::Image(ImageRefused::NotAnImage))),
        "{refused:?}"
    );
    // and: nothing is kept nor counted
    assert_eq!(bench.contents.pieces(), 0);
    assert_eq!(bench.usage().await, Usage::default());
}

#[tokio::test]
async fn a_deleted_asset_is_seen_no_more_its_pieces_gone_and_counted_nowhere() {
    // given: a photo in the loom
    let bench = a_bench();
    let (id, _) = bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &ada())
        .await
        .expect("an author uploads");
    let id = id.to_string();

    // when: Ada deletes it
    bench
        .assets
        .delete(&id, &ada())
        .await
        .expect("an author deletes");

    // then: nobody sees it
    assert!(is_not_found(&bench.assets.content(&id, None, &ada()).await));
    // and: none of its pieces is kept, nor is it counted
    assert_eq!(bench.contents.pieces(), 0);
    assert_eq!(bench.usage().await, Usage::default());
    // and: deleting it again is harmless
    bench
        .assets
        .delete(&id, &ada())
        .await
        .expect("deleting twice changes nothing");
}

#[tokio::test]
async fn an_id_of_no_asset_is_not_found() {
    // given: no asset
    let bench = a_bench();

    // expect: an id never given and one that is no id are both not there
    assert!(is_not_found(
        &bench
            .assets
            .content("asset_0123456789ABCDEFGHIJKL", None, &ada())
            .await
    ));
    assert!(matches!(
        bench.assets.content("not an id", None, &ada()).await,
        Err(AssetsError::InvalidId(_))
    ));
}

#[tokio::test]
async fn an_account_uploads_a_photo_of_its_own_counted_against_it_and_no_project() {
    // given: Ada and a limit on each account
    let bench = a_bench_limited(Limits {
        each_account: Some(1_000),
        ..Limits::default()
    });

    // when: she uploads a photo of her own
    let (id, standing) = bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &ada())
        .await
        .expect("an account uploads its own");

    // then: it is hers
    let asset = standing.state;
    assert_eq!(
        asset.owner(),
        &Owner::Account(AccountId::from("account_ada"))
    );
    // and: the installation and Ada hold it, no project
    let total = asset.total_size();
    assert_eq!(
        bench.usage().await,
        Usage {
            instance: total,
            account: total,
            project: 0,
        }
    );
    // and: she sees it
    assert!(
        bench
            .assets
            .content(&id.to_string(), None, &ada())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn an_account_s_own_photo_is_changed_by_it_alone() {
    // given: a photo of Ada's own
    let bench = a_bench();
    let (id, _) = bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &ada())
        .await
        .expect("an account uploads its own");
    let id = id.to_string();

    // when: Bo deletes it, and then Ada
    let by_bo = bench.assets.delete(&id, &bo()).await;
    let by_ada = bench.assets.delete(&id, &ada()).await;

    // then: Bo finds nothing to delete, and Ada deletes it
    assert!(is_not_found(&by_bo), "{by_bo:?}");
    assert!(by_ada.is_ok(), "{by_ada:?}");
}

#[tokio::test]
async fn nobody_keeps_photos_of_their_own_without_accounts() {
    // given: the instance without accounts
    let bench = a_bench();

    // when: a photo is uploaded as one's own
    let refused = bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &Agent::Local)
        .await;

    // then: it is refused, and nothing was made of it
    assert!(
        matches!(refused, Err(AssetsError::NoAccount)),
        "{refused:?}"
    );
    assert_eq!(bench.images.asked(), 0);
    assert_eq!(bench.contents.pieces(), 0);
}
