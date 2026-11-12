use assets_core::{
    AssetCatalog, AssetId, AssetSummary, CatalogError, Format, Owner, ProjectLink, Usage,
};
use eventsourcing::AccountId;
use transacting::{Failure, Transactions};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: AssetCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    async fn cleanup(self);
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn ada() -> AccountId {
    AccountId::from("account_ada")
}

fn grace() -> AccountId {
    AccountId::from("account_grace")
}

fn the_loom() -> ProjectLink {
    ProjectLink::from("project_loom")
}

fn the_weft() -> ProjectLink {
    ProjectLink::from("project_weft")
}

fn an_asset(seconds: i64) -> AssetId {
    AssetId::generate(
        ids::OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment"),
    )
}

fn a_summary(
    id: AssetId,
    owner: Owner,
    uploader: Option<AccountId>,
    total_size: u64,
) -> AssetSummary {
    AssetSummary {
        id,
        owner,
        uploader,
        format: Format::Jpeg,
        width: 1_280,
        height: 1_024,
        original_size: total_size / 2,
        total_size,
        uploaded_at: ids::OffsetDateTime::from_unix_timestamp(1_700_000_000)
            .expect("a plausible moment"),
    }
}

async fn remembered<W: Workbench>(bench: &W, summary: &AssetSummary) {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store()
        .remember(&mut transaction, summary)
        .await
        .expect("remembering should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn forgotten<W: Workbench>(bench: &W, id: &AssetId) {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store()
        .forget(&mut transaction, id)
        .await
        .expect("forgetting should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn usage<W: Workbench>(
    bench: &W,
    uploader: Option<&AccountId>,
    project: Option<&ProjectLink>,
) -> Usage {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .map_err(unreachable)
        .expect("beginning should succeed");
    let usage = bench
        .store()
        .usage_within(&mut transaction, uploader, project)
        .await
        .expect("asking should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    usage
}

pub async fn nothing_is_used_where_nothing_is_kept<W: Workbench>(bench: &W) {
    // expect: nothing used anywhere
    assert_eq!(
        usage(bench, Some(&ada()), Some(&the_loom())).await,
        Usage::default()
    );
}

pub async fn what_each_bound_holds_is_the_sum_of_its_assets<W: Workbench>(bench: &W) {
    // given: Ada uploading into the loom twice and into the weft once, Grace into the loom, and an account's own picture
    for (seconds, owner, uploader, total_size) in [
        (1, Owner::Project(the_loom()), Some(ada()), 100),
        (2, Owner::Project(the_loom()), Some(ada()), 20),
        (3, Owner::Project(the_weft()), Some(ada()), 3),
        (4, Owner::Project(the_loom()), Some(grace()), 4_000),
        (5, Owner::Account(ada()), Some(ada()), 50_000),
    ] {
        remembered(
            bench,
            &a_summary(an_asset(seconds), owner, uploader, total_size),
        )
        .await;
    }

    // expect: Ada's upload into the loom weighed against everything, all Ada uploaded, and all the loom holds
    assert_eq!(
        usage(bench, Some(&ada()), Some(&the_loom())).await,
        Usage {
            instance: 54_123,
            account: 50_123,
            project: 4_120,
        },
        "an account's own picture counts against the account and the installation, and no project"
    );
    // and: Grace's into the weft
    assert_eq!(
        usage(bench, Some(&grace()), Some(&the_weft())).await,
        Usage {
            instance: 54_123,
            account: 4_000,
            project: 3,
        }
    );
    // and: an upload by no account into no project counts the installation alone
    assert_eq!(
        usage(bench, None, None).await,
        Usage {
            instance: 54_123,
            account: 0,
            project: 0,
        },
        "what nobody uploaded is no account's"
    );
}

pub async fn an_asset_forgotten_counts_no_more_and_remembered_again_counts_once<W: Workbench>(
    bench: &W,
) {
    // given: two of Ada's assets in the loom
    let gone = an_asset(1);
    let kept = an_asset(2);
    remembered(
        bench,
        &a_summary(gone, Owner::Project(the_loom()), Some(ada()), 100),
    )
    .await;
    remembered(
        bench,
        &a_summary(kept, Owner::Project(the_loom()), Some(ada()), 20),
    )
    .await;
    // and: the second remembered again, as when a message is heard twice
    remembered(
        bench,
        &a_summary(kept, Owner::Project(the_loom()), Some(ada()), 20),
    )
    .await;

    // when: the first is forgotten, twice, and one never kept too
    forgotten(bench, &gone).await;
    forgotten(bench, &gone).await;
    forgotten(bench, &an_asset(3)).await;

    // then: the other alone counts, once
    assert_eq!(
        usage(bench, Some(&ada()), Some(&the_loom())).await,
        Usage {
            instance: 20,
            account: 20,
            project: 20,
        }
    );
}

pub async fn what_is_remembered_in_a_transaction_that_is_dropped_counts_nothing<W: Workbench>(
    bench: &W,
) {
    // given: an asset remembered
    let kept = an_asset(1);
    remembered(
        bench,
        &a_summary(kept, Owner::Project(the_loom()), Some(ada()), 100),
    )
    .await;

    // when: in a transaction that is dropped, another is remembered and the first forgotten
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store()
        .remember(
            &mut transaction,
            &a_summary(an_asset(2), Owner::Project(the_loom()), Some(ada()), 7),
        )
        .await
        .expect("remembering should succeed");
    bench
        .store()
        .forget(&mut transaction, &kept)
        .await
        .expect("forgetting should succeed");
    drop(transaction);

    // then: the first alone counts
    assert_eq!(
        usage(bench, Some(&ada()), Some(&the_loom())).await.instance,
        100
    );
}

pub async fn an_asset_remembered_again_counts_as_it_stands_now_and_nowhere_it_was<W: Workbench>(
    bench: &W,
) {
    // given: Ada's asset of a hundred in the loom
    let asset = an_asset(1);
    remembered(
        bench,
        &a_summary(asset, Owner::Project(the_loom()), Some(ada()), 100),
    )
    .await;

    // when: it is remembered again as Grace's, of seven, in the weft
    remembered(
        bench,
        &a_summary(asset, Owner::Project(the_weft()), Some(grace()), 7),
    )
    .await;

    // then: it counts as Grace's in the weft alone
    assert_eq!(
        usage(bench, Some(&ada()), Some(&the_loom())).await,
        Usage {
            instance: 7,
            account: 0,
            project: 0,
        },
        "nothing of what it was is left in Ada's total, nor the loom's"
    );
    assert_eq!(
        usage(bench, Some(&grace()), Some(&the_weft())).await,
        Usage {
            instance: 7,
            account: 7,
            project: 7,
        }
    );

    // when: it is forgotten
    forgotten(bench, &asset).await;

    // then: it counts nowhere
    assert_eq!(
        usage(bench, Some(&grace()), Some(&the_weft())).await,
        Usage::default()
    );
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::catalog_conformance_case!($workbench, nothing_is_used_where_nothing_is_kept);
        $crate::catalog_conformance_case!(
            $workbench,
            what_each_bound_holds_is_the_sum_of_its_assets
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_asset_forgotten_counts_no_more_and_remembered_again_counts_once
        );
        $crate::catalog_conformance_case!(
            $workbench,
            what_is_remembered_in_a_transaction_that_is_dropped_counts_nothing
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_asset_remembered_again_counts_as_it_stands_now_and_nowhere_it_was
        );
    };
}
