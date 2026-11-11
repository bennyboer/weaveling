use assets_core::{AssetId, Contents, Piece};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Contents;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

fn an_asset(seconds: i64) -> AssetId {
    AssetId::generate(
        ids::OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment"),
    )
}

fn bytes_of_every_value() -> Vec<u8> {
    (0..=255).cycle().take(1_000_003).collect()
}

async fn read(contents: &impl Contents, asset: &AssetId, piece: Piece) -> Option<Vec<u8>> {
    contents
        .read(asset, piece)
        .await
        .expect("reading should succeed")
}

pub async fn each_piece_kept_reads_back_as_it_was_kept(contents: &impl Contents) {
    // given: an asset's original, of every byte value and a megabyte long, and two of its variants
    let asset = an_asset(1_000);
    let original = bytes_of_every_value();
    for (piece, bytes) in [
        (Piece::Original, original.clone()),
        (Piece::Width(384), b"a small one".to_vec()),
        (Piece::Width(768), b"a larger one".to_vec()),
    ] {
        contents
            .keep(&asset, piece, &bytes)
            .await
            .expect("keeping should succeed");
    }

    // expect: each reads back as it was kept
    assert_eq!(
        read(contents, &asset, Piece::Original).await,
        Some(original)
    );
    assert_eq!(
        read(contents, &asset, Piece::Width(384)).await.as_deref(),
        Some(&b"a small one"[..])
    );
    assert_eq!(
        read(contents, &asset, Piece::Width(768)).await.as_deref(),
        Some(&b"a larger one"[..])
    );
}

pub async fn a_piece_never_kept_reads_as_nothing(contents: &impl Contents) {
    // given: an asset with its original kept
    let asset = an_asset(1_000);
    contents
        .keep(&asset, Piece::Original, b"the original")
        .await
        .expect("keeping should succeed");

    // expect: a width never made reads as nothing
    assert_eq!(read(contents, &asset, Piece::Width(1_536)).await, None);
    // and: so does another asset's original
    assert_eq!(
        read(contents, &an_asset(2_000), Piece::Original).await,
        None
    );
}

pub async fn a_piece_kept_again_stands_in_place_of_the_one_before(contents: &impl Contents) {
    // given: a variant kept
    let asset = an_asset(1_000);
    contents
        .keep(&asset, Piece::Width(384), b"made first, and longer")
        .await
        .expect("keeping should succeed");

    // when: it is kept again, shorter
    contents
        .keep(&asset, Piece::Width(384), b"made again")
        .await
        .expect("keeping again should succeed");

    // then: the second is read, nothing of the first left
    assert_eq!(
        read(contents, &asset, Piece::Width(384)).await.as_deref(),
        Some(&b"made again"[..])
    );
}

pub async fn an_asset_forgotten_takes_every_piece_along_and_leaves_the_others(
    contents: &impl Contents,
) {
    // given: two assets, each with an original and a variant
    let gone = an_asset(1_000);
    let kept = an_asset(2_000);
    for asset in [&gone, &kept] {
        for piece in [Piece::Original, Piece::Width(384)] {
            contents
                .keep(asset, piece, asset.to_string().as_bytes())
                .await
                .expect("keeping should succeed");
        }
    }

    // when: the first is forgotten
    contents
        .forget(&gone)
        .await
        .expect("forgetting should succeed");

    // then: none of its pieces is read
    assert_eq!(read(contents, &gone, Piece::Original).await, None);
    assert_eq!(read(contents, &gone, Piece::Width(384)).await, None);
    // and: the other's are
    assert!(read(contents, &kept, Piece::Original).await.is_some());
    assert!(read(contents, &kept, Piece::Width(384)).await.is_some());
    // and: forgetting it again, or one never kept, is no failure
    contents
        .forget(&gone)
        .await
        .expect("forgetting twice is harmless");
    contents
        .forget(&an_asset(3_000))
        .await
        .expect("forgetting what was never kept is harmless");
}

#[macro_export]
macro_rules! contents_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::contents_conformance_case!($workbench, each_piece_kept_reads_back_as_it_was_kept);
        $crate::contents_conformance_case!($workbench, a_piece_never_kept_reads_as_nothing);
        $crate::contents_conformance_case!(
            $workbench,
            a_piece_kept_again_stands_in_place_of_the_one_before
        );
        $crate::contents_conformance_case!(
            $workbench,
            an_asset_forgotten_takes_every_piece_along_and_leaves_the_others
        );
    };
}
