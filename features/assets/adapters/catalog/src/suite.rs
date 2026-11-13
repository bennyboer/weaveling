use assets_core::{
    AssetCatalog, AssetId, AssetSummary, CatalogError, Format, Held, Holders, Owner, Person,
    ProjectLink, Sorted, Upload, UploadMark, UploadsSorted, Usage,
};
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
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

fn bo() -> AccountId {
    AccountId::from("account_bo")
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

async fn held<W: Workbench>(
    bench: &W,
    holders: Holders,
    sorted: Sorted,
    after: Option<&Held>,
    at_most: u64,
) -> Vec<Held> {
    found(bench, holders, sorted, after, None, at_most).await
}

async fn found<W: Workbench>(
    bench: &W,
    holders: Holders,
    sorted: Sorted,
    after: Option<&Held>,
    matching: Option<&str>,
    at_most: u64,
) -> Vec<Held> {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .map_err(unreachable)
        .expect("beginning should succeed");
    let held = bench
        .store()
        .held_by(&mut transaction, holders, sorted, after, matching, at_most)
        .await
        .expect("asking should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    held
}

fn holding(id: &str, total: u64) -> Held {
    Held {
        id: id.to_owned(),
        total,
        name: None,
        email: None,
    }
}

fn named(id: &str, total: u64, name: &str, email: Option<&str>) -> Held {
    Held {
        name: Some(name.to_owned()),
        email: email.map(str::to_owned),
        ..holding(id, total)
    }
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

/// Each word on a name, as the assets' listeners tell the catalog.
enum Naming {
    Person(&'static str, &'static str, &'static str, u64),
    PersonGone(&'static str, u64, i64),
    Project(&'static str, &'static str, u64),
    ProjectGone(&'static str, u64, i64),
}

async fn named_as<W: Workbench>(bench: &W, namings: Vec<Naming>) {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .map_err(unreachable)
        .expect("beginning should succeed");
    for naming in namings {
        let store = bench.store();
        match naming {
            Naming::Person(account, name, email, version) => {
                store
                    .remember_person(
                        &mut transaction,
                        &Person {
                            account: AccountId::from(account),
                            name: name.to_owned(),
                            email: email.to_owned(),
                        },
                        Version::of(version),
                    )
                    .await
            }
            Naming::PersonGone(account, version, when) => {
                store
                    .forget_person(
                        &mut transaction,
                        &AccountId::from(account),
                        Version::of(version),
                        at(when),
                    )
                    .await
            }
            Naming::Project(project, name, version) => {
                store
                    .remember_project_name(
                        &mut transaction,
                        &ProjectLink::from(project),
                        Version::of(version),
                        name,
                    )
                    .await
            }
            Naming::ProjectGone(project, version, when) => {
                store
                    .forget_project_name(
                        &mut transaction,
                        &ProjectLink::from(project),
                        Version::of(version),
                        at(when),
                    )
                    .await
            }
        }
        .expect("naming should succeed");
    }
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn names_forgotten<W: Workbench>(bench: &W, before: i64, at_most: u64) -> u64 {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .map_err(unreachable)
        .expect("beginning should succeed");
    let forgotten = bench
        .store()
        .forget_tombstones_laid_before(&mut transaction, at(before), at_most)
        .await
        .expect("forgetting should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    forgotten
}

/// Ada and Bo named as the accounts told it, Grace not; the loom named as the projects told it, the weft not.
fn the_names() -> Vec<Naming> {
    vec![
        Naming::Person("account_ada", "Ada Lovelace", "ada@example.com", 1),
        Naming::Person("account_bo", "Bo Jürgensen", "bo@example.org", 1),
        Naming::Project("project_loom", "The Loom", 1),
    ]
}

/// Ada's 300 bytes in the loom and 50 in the weft, Bo's 300 of his own, Grace's 100 in the weft, and 70 uploaded by nobody into the loom.
async fn holdings_kept<W: Workbench>(bench: &W) {
    for (seconds, owner, uploader, total_size) in [
        (1, Owner::Project(the_loom()), Some(ada()), 300),
        (2, Owner::Project(the_weft()), Some(ada()), 50),
        (3, Owner::Account(bo()), Some(bo()), 300),
        (4, Owner::Project(the_weft()), Some(grace()), 100),
        (5, Owner::Project(the_loom()), None, 70),
    ] {
        remembered(
            bench,
            &a_summary(an_asset(seconds), owner, uploader, total_size),
        )
        .await;
    }
}

pub async fn nothing_is_held_where_nothing_is_kept<W: Workbench>(bench: &W) {
    // expect: no account and no project holds anything, either way listed
    for holders in [Holders::Accounts, Holders::Projects] {
        for sorted in [Sorted::MostUsed, Sorted::ById] {
            assert!(held(bench, holders, sorted, None, 10).await.is_empty());
        }
    }
}

pub async fn the_accounts_and_projects_are_listed_by_what_they_hold_or_by_id<W: Workbench>(
    bench: &W,
) {
    // given: what Ada, Bo and Grace uploaded, into projects and their own
    holdings_kept(bench).await;

    // expect: the accounts that uploaded, the most first and the ids in order where they hold as much
    assert_eq!(
        held(bench, Holders::Accounts, Sorted::MostUsed, None, 10).await,
        [
            holding("account_ada", 350),
            holding("account_bo", 300),
            holding("account_grace", 100),
        ]
    );
    // and: the projects, an account's own counting in none
    assert_eq!(
        held(bench, Holders::Projects, Sorted::MostUsed, None, 10).await,
        [holding("project_loom", 370), holding("project_weft", 150)]
    );
    assert_eq!(
        held(bench, Holders::Projects, Sorted::ById, None, 10).await,
        [holding("project_loom", 370), holding("project_weft", 150)]
    );
}

pub async fn holdings_are_listed_a_page_at_a_time_after_the_last_listed<W: Workbench>(bench: &W) {
    // given: what Ada, Bo and Grace uploaded
    holdings_kept(bench).await;

    // when: the accounts are listed two at a time, the most first, and by id
    let first = held(bench, Holders::Accounts, Sorted::MostUsed, None, 2).await;
    let rest = held(bench, Holders::Accounts, Sorted::MostUsed, first.last(), 2).await;
    let by_id = held(bench, Holders::Accounts, Sorted::ById, None, 1).await;
    let by_id_then = held(bench, Holders::Accounts, Sorted::ById, by_id.last(), 5).await;

    // then: each page goes on where the one before ended
    assert_eq!(
        first,
        [holding("account_ada", 350), holding("account_bo", 300)]
    );
    assert_eq!(rest, [holding("account_grace", 100)]);
    assert_eq!(by_id, [holding("account_ada", 350)]);
    assert_eq!(
        by_id_then,
        [holding("account_bo", 300), holding("account_grace", 100)]
    );
    // and: holding as much as the last listed, an id before it is not listed again
    assert_eq!(
        held(
            bench,
            Holders::Accounts,
            Sorted::MostUsed,
            Some(&holding("account_bo", 350)),
            5
        )
        .await,
        [holding("account_bo", 300), holding("account_grace", 100)]
    );
}

pub async fn whoever_holds_nothing_any_more_is_not_listed<W: Workbench>(bench: &W) {
    // given: Grace's one upload, forgotten
    let id = an_asset(9);
    remembered(
        bench,
        &a_summary(id, Owner::Project(the_weft()), Some(grace()), 100),
    )
    .await;
    forgotten(bench, &id).await;

    // expect: neither she nor the weft is listed
    assert!(
        held(bench, Holders::Accounts, Sorted::MostUsed, None, 10)
            .await
            .is_empty()
    );
    assert!(
        held(bench, Holders::Projects, Sorted::ById, None, 10)
            .await
            .is_empty()
    );
}

pub async fn holdings_are_named_as_heard_and_unnamed_where_nothing_was<W: Workbench>(bench: &W) {
    // given: what Ada, Bo and Grace uploaded, and the names heard
    holdings_kept(bench).await;
    named_as(bench, the_names()).await;

    // expect: the accounts named, with their addresses, Grace unnamed
    assert_eq!(
        held(bench, Holders::Accounts, Sorted::MostUsed, None, 10).await,
        [
            named("account_ada", 350, "Ada Lovelace", Some("ada@example.com")),
            named("account_bo", 300, "Bo Jürgensen", Some("bo@example.org")),
            holding("account_grace", 100),
        ]
    );
    // and: the projects named, the weft unnamed, and no project with an address
    assert_eq!(
        held(bench, Holders::Projects, Sorted::MostUsed, None, 10).await,
        [
            named("project_loom", 370, "The Loom", None),
            holding("project_weft", 150),
        ]
    );
}

pub async fn holdings_are_found_by_a_term_in_their_name_or_address_whatever_its_case<
    W: Workbench,
>(
    bench: &W,
) {
    // given: what Ada, Bo and Grace uploaded, and the names heard
    holdings_kept(bench).await;
    named_as(bench, the_names()).await;
    let accounts = |term: &'static str| async move {
        found(
            bench,
            Holders::Accounts,
            Sorted::MostUsed,
            None,
            Some(term),
            10,
        )
        .await
        .into_iter()
        .map(|held| held.id)
        .collect::<Vec<_>>()
    };

    // expect: a part of a name, of an address, in any case, an umlaut alike
    assert_eq!(accounts("LOVE").await, ["account_ada"]);
    assert_eq!(accounts("example.org").await, ["account_bo"]);
    assert_eq!(accounts("JÜR").await, ["account_bo"]);
    assert_eq!(accounts("example").await, ["account_ada", "account_bo"]);
    // and: what nobody's name holds, and the signs a pattern is written in, find nobody
    assert!(accounts("hopper").await.is_empty());
    assert!(accounts("%").await.is_empty());
    assert!(accounts("_").await.is_empty());
    // and: a project by its name, a page at a time
    assert_eq!(
        found(
            bench,
            Holders::Projects,
            Sorted::ById,
            None,
            Some("loom"),
            10
        )
        .await,
        [named("project_loom", 370, "The Loom", None)]
    );
    assert_eq!(
        found(
            bench,
            Holders::Accounts,
            Sorted::MostUsed,
            Some(&holding("account_ada", 350)),
            Some("example"),
            10
        )
        .await,
        [named(
            "account_bo",
            300,
            "Bo Jürgensen",
            Some("bo@example.org")
        )]
    );
}

pub async fn the_newest_name_heard_holds_and_an_older_one_heard_late_changes_nothing<
    W: Workbench,
>(
    bench: &W,
) {
    // given: what Ada uploaded, the loom renamed, and Ada's name told anew
    holdings_kept(bench).await;
    named_as(
        bench,
        vec![
            Naming::Project("project_loom", "The Loom", 1),
            Naming::Project("project_loom", "The Great Loom", 3),
            Naming::Person("account_ada", "Ada King", "ada@example.com", 4),
        ],
    )
    .await;

    // when: older names are heard late
    named_as(
        bench,
        vec![
            Naming::Project("project_loom", "The Loom", 2),
            Naming::Person("account_ada", "Ada Lovelace", "ada@example.com", 1),
        ],
    )
    .await;

    // then: the newest are kept
    assert_eq!(
        found(
            bench,
            Holders::Projects,
            Sorted::ById,
            None,
            Some("great"),
            10
        )
        .await,
        [named("project_loom", 370, "The Great Loom", None)]
    );
    assert_eq!(
        found(
            bench,
            Holders::Accounts,
            Sorted::ById,
            None,
            Some("ada"),
            10
        )
        .await
        .first()
        .and_then(|held| held.name.clone()),
        Some("Ada King".to_owned())
    );
}

pub async fn a_name_forgotten_is_neither_shown_nor_found_and_not_brought_back_late<W: Workbench>(
    bench: &W,
) {
    // given: what Ada uploaded and the names heard
    holdings_kept(bench).await;
    named_as(bench, the_names()).await;

    // when: Ada's account and the loom are deleted, and their names heard again late
    named_as(
        bench,
        vec![
            Naming::PersonGone("account_ada", 5, 1_000),
            Naming::ProjectGone("project_loom", 5, 1_000),
        ],
    )
    .await;
    named_as(
        bench,
        vec![
            Naming::Person("account_ada", "Ada Lovelace", "ada@example.com", 2),
            Naming::Project("project_loom", "The Loom", 2),
        ],
    )
    .await;

    // then: neither is named nor found any more
    let accounts = held(bench, Holders::Accounts, Sorted::ById, None, 10).await;
    assert_eq!(accounts[0], holding("account_ada", 350));
    assert!(
        found(
            bench,
            Holders::Accounts,
            Sorted::ById,
            None,
            Some("ada"),
            10
        )
        .await
        .is_empty()
    );
    assert_eq!(
        held(bench, Holders::Projects, Sorted::ById, None, 10).await[0],
        holding("project_loom", 370)
    );
}

pub async fn names_forgotten_long_ago_are_let_go_and_those_lately_still_guard<W: Workbench>(
    bench: &W,
) {
    // given: what Ada and Bo uploaded, Ada's account deleted long ago and Bo's lately
    holdings_kept(bench).await;
    named_as(
        bench,
        vec![
            Naming::PersonGone("account_ada", 5, 1_000),
            Naming::PersonGone("account_bo", 5, 5_000),
            Naming::ProjectGone("project_weft", 5, 1_000),
        ],
    )
    .await;

    // when: what was forgotten before a moment between is let go
    let forgotten = names_forgotten(bench, 3_000, 10).await;

    // then: Ada's and the weft's are let go, Bo's not
    assert_eq!(forgotten, 2);
    assert_eq!(names_forgotten(bench, 3_000, 10).await, 0);
    // and: an older word about Ada is taken in once nothing guards against it, and about Bo not
    named_as(
        bench,
        vec![
            Naming::Person("account_ada", "Ada Lovelace", "ada@example.com", 2),
            Naming::Person("account_bo", "Bo Jürgensen", "bo@example.org", 2),
        ],
    )
    .await;
    assert_eq!(
        found(
            bench,
            Holders::Accounts,
            Sorted::ById,
            None,
            Some("example"),
            10
        )
        .await
        .into_iter()
        .map(|held| held.id)
        .collect::<Vec<_>>(),
        ["account_ada"]
    );
}

async fn uploads<W: Workbench>(
    bench: &W,
    account: &AccountId,
    sorted: UploadsSorted,
    after: Option<&UploadMark>,
    at_most: u64,
) -> Vec<Upload> {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .map_err(unreachable)
        .expect("beginning should succeed");
    let uploads = bench
        .store()
        .uploaded_by(&mut transaction, account, sorted, after, at_most)
        .await
        .expect("asking should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    uploads
}

fn ids(uploads: &[Upload]) -> Vec<AssetId> {
    uploads.iter().map(|upload| upload.id).collect()
}

/// Two assets alike in how they are sorted, in the order of their ids as written.
fn in_id_order(one: AssetId, other: AssetId) -> [AssetId; 2] {
    if one.to_string() < other.to_string() {
        [one, other]
    } else {
        [other, one]
    }
}

/// Ada's 300 bytes in the loom, uploaded first; her own 300 next; 50 in the weft and 120 of her own last, at the same moment; and Bo's 500 of his own.
async fn adas_uploads_kept<W: Workbench>(bench: &W) -> [AssetId; 4] {
    let adas = [an_asset(1), an_asset(2), an_asset(3), an_asset(4)];
    for (id, owner, uploader, total_size, uploaded) in [
        (adas[0], Owner::Project(the_loom()), ada(), 300, 10),
        (adas[1], Owner::Account(ada()), ada(), 300, 20),
        (adas[2], Owner::Project(the_weft()), ada(), 50, 30),
        (adas[3], Owner::Account(ada()), ada(), 120, 30),
        (an_asset(5), Owner::Account(bo()), bo(), 500, 40),
    ] {
        remembered(
            bench,
            &AssetSummary {
                uploaded_at: at(1_700_000_000 + uploaded),
                ..a_summary(id, owner, Some(uploader), total_size)
            },
        )
        .await;
    }

    adas
}

pub async fn an_account_s_uploads_are_listed_the_largest_or_the_newest_first<W: Workbench>(
    bench: &W,
) {
    // given: what Ada and Bo uploaded
    let [loom, own, weft, own_last] = adas_uploads_kept(bench).await;

    // expect: Ada's alone, the largest first, the ids in order where they take as much
    let largest = in_id_order(loom, own);
    assert_eq!(
        ids(&uploads(bench, &ada(), UploadsSorted::Largest, None, 10).await),
        [largest[0], largest[1], own_last, weft]
    );
    // and: the newest first, the ids in order where they were uploaded at once
    let newest = in_id_order(weft, own_last);
    assert_eq!(
        ids(&uploads(bench, &ada(), UploadsSorted::Newest, None, 10).await),
        [newest[0], newest[1], own, loom]
    );
    // and: nothing of whoever uploaded nothing
    assert!(
        uploads(bench, &grace(), UploadsSorted::Newest, None, 10)
            .await
            .is_empty()
    );
}

pub async fn an_upload_is_listed_with_whose_it_is_its_picture_and_when<W: Workbench>(bench: &W) {
    // given: what Ada and Bo uploaded, the loom named
    let [loom, ..] = adas_uploads_kept(bench).await;
    named_as(bench, vec![Naming::Project("project_loom", "The Loom", 1)]).await;

    // when: Ada's are listed, the oldest last
    let listed = uploads(bench, &ada(), UploadsSorted::Newest, None, 10).await;

    // then: the upload into the loom says all of it, the loom by its name
    assert_eq!(
        listed.last(),
        Some(&Upload {
            id: loom,
            owner: Owner::Project(the_loom()),
            project_name: Some("The Loom".to_owned()),
            format: Format::Jpeg,
            width: 1_280,
            height: 1_024,
            total_size: 300,
            uploaded_at: at(1_700_000_010),
        })
    );
    // and: the weft, never named, and her own are named by nothing
    assert!(
        listed
            .iter()
            .filter(|upload| upload.id != loom)
            .all(|upload| upload.project_name.is_none())
    );
}

pub async fn a_project_s_name_forgotten_names_its_uploads_no_more<W: Workbench>(bench: &W) {
    // given: what Ada uploaded, the loom named and then deleted
    adas_uploads_kept(bench).await;
    named_as(
        bench,
        vec![
            Naming::Project("project_loom", "The Loom", 1),
            Naming::ProjectGone("project_loom", 2, 1_700_000_100),
        ],
    )
    .await;

    // expect: no upload is named
    assert!(
        uploads(bench, &ada(), UploadsSorted::Largest, None, 10)
            .await
            .iter()
            .all(|upload| upload.project_name.is_none())
    );
}

pub async fn uploads_are_listed_a_page_at_a_time_after_the_last_listed<W: Workbench>(bench: &W) {
    // given: what Ada and Bo uploaded
    adas_uploads_kept(bench).await;

    for sorted in [UploadsSorted::Largest, UploadsSorted::Newest] {
        // when: Ada's are listed one at a time, each page going on from the last
        let mut paged = Vec::new();
        let mut after: Option<UploadMark> = None;
        loop {
            let page = uploads(bench, &ada(), sorted, after.as_ref(), 1).await;
            let Some(last) = page.last() else {
                break;
            };
            after = Some(UploadMark::from(last));
            paged.extend(ids(&page));
        }

        // then: the pages hold every one once, in the order of a single page
        assert_eq!(
            paged,
            ids(&uploads(bench, &ada(), sorted, None, 10).await),
            "{sorted:?}"
        );
    }
}

pub async fn an_upload_forgotten_is_listed_no_more<W: Workbench>(bench: &W) {
    // given: what Ada uploaded, her upload into the loom forgotten
    let [loom, ..] = adas_uploads_kept(bench).await;
    forgotten(bench, &loom).await;

    // expect: three are listed, none of them the loom's
    let listed = ids(&uploads(bench, &ada(), UploadsSorted::Largest, None, 10).await);
    assert_eq!(listed.len(), 3);
    assert!(!listed.contains(&loom));
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
        $crate::catalog_conformance_case!($workbench, nothing_is_held_where_nothing_is_kept);
        $crate::catalog_conformance_case!(
            $workbench,
            the_accounts_and_projects_are_listed_by_what_they_hold_or_by_id
        );
        $crate::catalog_conformance_case!(
            $workbench,
            holdings_are_listed_a_page_at_a_time_after_the_last_listed
        );
        $crate::catalog_conformance_case!($workbench, whoever_holds_nothing_any_more_is_not_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            holdings_are_named_as_heard_and_unnamed_where_nothing_was
        );
        $crate::catalog_conformance_case!(
            $workbench,
            holdings_are_found_by_a_term_in_their_name_or_address_whatever_its_case
        );
        $crate::catalog_conformance_case!(
            $workbench,
            the_newest_name_heard_holds_and_an_older_one_heard_late_changes_nothing
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_name_forgotten_is_neither_shown_nor_found_and_not_brought_back_late
        );
        $crate::catalog_conformance_case!(
            $workbench,
            names_forgotten_long_ago_are_let_go_and_those_lately_still_guard
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_account_s_uploads_are_listed_the_largest_or_the_newest_first
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_upload_is_listed_with_whose_it_is_its_picture_and_when
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_s_name_forgotten_names_its_uploads_no_more
        );
        $crate::catalog_conformance_case!(
            $workbench,
            uploads_are_listed_a_page_at_a_time_after_the_last_listed
        );
        $crate::catalog_conformance_case!($workbench, an_upload_forgotten_is_listed_no_more);
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
