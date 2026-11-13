use assets_contract::{AccountsStorageDTO, InstanceStorageDTO, ProjectsStorageDTO, refused};
use assets_core::{AssetsError, Limits, Person, ProjectLink, Sorted};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use eventsourcing::{AccountId, Agent, Version};
use serde_json::Value;

use crate::bench::{self, Bench, THE_LOOM};

const A_PHOTO: &[u8] = b"the bytes of a photo, as uploaded";

/// Every account held to a megabyte, the instance to a gigabyte and each project to ten megabytes; Ada running it, given two megabytes of her own, with two photos in the loom; Bo with a portrait of his own; each named as the accounts and the projects told it.
async fn an_instance_holding_some() -> (Bench, u64) {
    let bench = bench::a_bench_limited(Limits {
        instance: Some(1 << 30),
        each_account: Some(1 << 20),
        each_project: Some(10 << 20),
        ..Limits::default()
    })
    .await;
    bench.ada_runs_the_instance().await;
    bench.adas_own_limit(Some(2 << 20)).await;
    let mut one = 0;
    for _ in 0..2 {
        one = bench
            .assets
            .upload_into(THE_LOOM, A_PHOTO.to_vec(), &bench::ada())
            .await
            .expect("an author uploads")
            .1
            .state
            .total_size();
    }
    bench
        .assets
        .upload_as_own(A_PHOTO.to_vec(), &bench::bo())
        .await
        .expect("an account uploads its own");
    for (account, name, email) in [
        ("account_ada", "Ada Lovelace", "ada@example.com"),
        ("account_bo", "Bo Jansen", "bo@example.com"),
    ] {
        bench
            .assets
            .remember_person(
                None,
                &Person {
                    account: AccountId::from(account),
                    name: name.to_owned(),
                    email: email.to_owned(),
                },
                Version::of(1),
            )
            .await
            .expect("remembering should succeed");
    }
    bench
        .assets
        .remember_project_name(None, &ProjectLink::from(THE_LOOM), Version::of(1), "Loom")
        .await
        .expect("remembering should succeed");

    (bench, one)
}

fn serving(bench: &Bench, acting: Agent) -> TestServer {
    TestServer::new(assets_rest::router(bench.assets.clone()).layer(Extension(acting)))
}

#[tokio::test]
async fn the_operator_is_told_what_the_instance_holds_and_the_most_it_may() {
    // given: an instance holding three photos, Ada running it
    let (bench, one) = an_instance_holding_some().await;

    // when: she asks what the instance holds
    let storage = bench
        .assets
        .instance_storage(&bench::ada())
        .await
        .expect("an operator is told");

    // then: all three, out of the instance's gigabyte
    assert_eq!(storage.used, 3 * one);
    assert_eq!(storage.most, Some(1 << 30));
}

#[tokio::test]
async fn the_operator_is_told_each_account_s_holding_the_most_first_and_its_limit() {
    // given: an instance holding three photos, Ada running it
    let (bench, one) = an_instance_holding_some().await;

    // when: she lists the accounts the most first, one at a time
    let first = bench
        .assets
        .accounts_storage(Sorted::MostUsed, None, None, 1, &bench::ada())
        .await
        .expect("an operator is told");
    let then = bench
        .assets
        .accounts_storage(
            Sorted::MostUsed,
            first.next.as_ref(),
            None,
            1,
            &bench::ada(),
        )
        .await
        .expect("an operator is told");

    // then: Ada first, by name and address, holding two photos out of her own two megabytes
    let ada = &first.listed[0];
    assert_eq!(ada.account, AccountId::from("account_ada"));
    assert_eq!(
        (ada.name.as_deref(), ada.email.as_deref()),
        (Some("Ada Lovelace"), Some("ada@example.com"))
    );
    assert_eq!(ada.used, 2 * one);
    assert_eq!(ada.most, Some(2 << 20));
    assert!(ada.own_limit);
    // and: then Bo, holding one out of every account's megabyte
    let bo = &then.listed[0];
    assert_eq!(bo.account, AccountId::from("account_bo"));
    assert_eq!(
        (bo.used, bo.most, bo.own_limit),
        (one, Some(1 << 20), false)
    );
}

#[tokio::test]
async fn the_operator_is_told_each_project_s_name_holding_and_owners() {
    // given: an instance holding three photos, two in the loom Ada owns and Bo reads
    let (bench, one) = an_instance_holding_some().await;

    // when: she lists the projects
    let listed = bench
        .assets
        .projects_storage(Sorted::ById, None, None, 10, &bench::ada())
        .await
        .expect("an operator is told");

    // then: the loom alone, Bo's own portrait in no project, by its name, with its holding, its limit and Ada its owner
    assert_eq!(listed.listed.len(), 1);
    let loom = &listed.listed[0];
    assert_eq!(loom.project, ProjectLink::from(THE_LOOM));
    assert_eq!(loom.name.as_deref(), Some("Loom"));
    assert_eq!((loom.used, loom.most), (2 * one, Some(10 << 20)));
    assert_eq!(loom.owners, [AccountId::from("account_ada")]);
    // and: nothing more to go on to
    assert_eq!(listed.next, None);
}

#[tokio::test]
async fn the_operator_finds_an_account_or_a_project_by_a_term() {
    // given: an instance holding three photos, Ada running it
    let (bench, _) = an_instance_holding_some().await;
    let accounts_found = |term: &'static str| {
        let bench = &bench;
        async move {
            bench
                .assets
                .accounts_storage(Sorted::MostUsed, None, Some(term), 10, &bench::ada())
                .await
                .expect("an operator is told")
                .listed
                .into_iter()
                .map(|account| account.account.to_string())
                .collect::<Vec<_>>()
        }
    };

    // expect: an account found by its name or address, whatever its case
    assert_eq!(accounts_found("JANSEN").await, ["account_bo"]);
    assert_eq!(accounts_found("ada@").await, ["account_ada"]);
    // and: by what both hold, both
    assert_eq!(accounts_found("example").await.len(), 2);
    // and: a term of nothing but spaces searches for nothing
    assert_eq!(accounts_found("  ").await.len(), 2);
    // and: a project by its name, and none by a name it does not hold
    for (term, found) in [("loo", 1), ("weft", 0)] {
        assert_eq!(
            bench
                .assets
                .projects_storage(Sorted::MostUsed, None, Some(term), 10, &bench::ada())
                .await
                .expect("an operator is told")
                .listed
                .len(),
            found,
            "{term}"
        );
    }
}

#[tokio::test]
async fn whoever_runs_nothing_is_told_nothing_of_what_is_stored() {
    // given: an instance holding three photos, Bo not running it
    let (bench, _) = an_instance_holding_some().await;
    let bo = bench::bo();

    // when: he asks what the instance, the accounts and the projects hold
    let instance = bench.assets.instance_storage(&bo).await;
    let accounts = bench
        .assets
        .accounts_storage(Sorted::MostUsed, None, None, 10, &bo)
        .await;
    let projects = bench
        .assets
        .projects_storage(Sorted::MostUsed, None, None, 10, &bo)
        .await;

    // then: each is refused
    assert!(
        matches!(instance, Err(AssetsError::NotAnOperator)),
        "{instance:?}"
    );
    assert!(
        matches!(accounts, Err(AssetsError::NotAnOperator)),
        "{accounts:?}"
    );
    assert!(
        matches!(projects, Err(AssetsError::NotAnOperator)),
        "{projects:?}"
    );
}

#[tokio::test]
async fn without_accounts_the_one_working_is_told_what_is_stored_and_no_account_s_limit() {
    // given: the instance without accounts, a photo in the loom
    let bench = bench::a_bench_without_accounts(Limits::default()).await;
    bench
        .assets
        .upload_into(THE_LOOM, A_PHOTO.to_vec(), &Agent::Local)
        .await
        .expect("everyone uploads where there are no accounts");

    // when: the one working asks what the instance and the projects hold
    let instance = bench
        .assets
        .instance_storage(&Agent::Local)
        .await
        .expect("the one working runs the instance");
    let projects = bench
        .assets
        .projects_storage(Sorted::MostUsed, None, None, 10, &Agent::Local)
        .await
        .expect("the one working runs the instance");

    // then: the photo is counted, against no limit, in the loom
    assert!(instance.used > 0);
    assert_eq!(instance.most, None);
    assert_eq!(projects.listed[0].used, instance.used);
}

#[tokio::test]
async fn the_routes_list_a_page_and_go_on_from_its_cursor() {
    // given: an instance holding three photos, the routes for Ada and for Bo
    let (bench, one) = an_instance_holding_some().await;
    let (hers, his) = (serving(&bench, bench::ada()), serving(&bench, bench::bo()));

    // when: Ada asks what the instance, the accounts and the projects hold
    let instance: InstanceStorageDTO = hers.get("/assets/storage").await.json();
    let accounts: AccountsStorageDTO = hers.get("/assets/storage/accounts").await.json();
    let projects: ProjectsStorageDTO = hers.get("/assets/storage/projects?sort=id").await.json();
    // and: the accounts again, going on from Ada's holding
    let after_ada: AccountsStorageDTO = hers
        .get(&format!(
            "/assets/storage/accounts?sort=most-used&after={}.account_ada",
            2 * one
        ))
        .await
        .json();

    // then: each is told
    assert_eq!(instance.used, 3 * one);
    assert_eq!(
        accounts
            .accounts
            .iter()
            .map(|account| account.account.as_str())
            .collect::<Vec<_>>(),
        ["account_ada", "account_bo"]
    );
    assert_eq!(accounts.next, None, "a page holds fifty");
    assert_eq!(projects.projects[0].owners, ["account_ada"]);
    assert_eq!(after_ada.accounts.len(), 1);
    assert_eq!(after_ada.accounts[0].account, "account_bo");
    // when: she searches the accounts and the projects
    let found: AccountsStorageDTO = hers
        .get("/assets/storage/accounts?search=lovelace")
        .await
        .json();
    let none: ProjectsStorageDTO = hers
        .get("/assets/storage/projects?search=weft")
        .await
        .json();
    // then: Ada alone by her name and address, and no project
    assert_eq!(found.accounts.len(), 1);
    assert_eq!(
        (
            found.accounts[0].name.as_deref(),
            found.accounts[0].email.as_deref()
        ),
        (Some("Ada Lovelace"), Some("ada@example.com"))
    );
    assert!(none.projects.is_empty());
    assert_eq!(projects.projects[0].name.as_deref(), Some("Loom"));
    // and: an unknown sort, a cursor that is none, and Bo, are refused
    let unknown = hers.get("/assets/storage/accounts?sort=largest").await;
    unknown.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(unknown.json::<Value>()["type"], refused::SORT_UNKNOWN);
    let none = hers.get("/assets/storage/accounts?after=account_ada").await;
    none.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(none.json::<Value>()["type"], refused::CURSOR_INVALID);
    let by_bo = his.get("/assets/storage").await;
    by_bo.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(by_bo.json::<Value>()["type"], refused::NOT_AN_OPERATOR);
}

#[tokio::test]
async fn a_full_page_tells_where_the_next_goes_on_from() {
    // given: an instance holding three photos, Ada running it
    let (bench, one) = an_instance_holding_some().await;

    // when: she lists the accounts a page of two at a time
    let full = bench
        .assets
        .accounts_storage(Sorted::MostUsed, None, None, 2, &bench::ada())
        .await
        .expect("an operator is told");

    // then: the page is full, and the next goes on from its last
    let next = full.next.expect("a full page goes on");
    assert_eq!((next.id.as_str(), next.total), ("account_bo", one));
}
