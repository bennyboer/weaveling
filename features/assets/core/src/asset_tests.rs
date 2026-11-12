use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateContext, AggregateId, EventMetadata, Version,
};
use projects_contract::context;
use time::OffsetDateTime;

use crate::asset::*;

fn ada() -> Agent {
    Agent::Account("account_ada".into())
}

fn stamped(version: u64, occurred_at: OffsetDateTime) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("asset_1"),
        kind: KIND,
        version: Version::of(version),
        agent: ada(),
        occurred_at,
        is_snapshot: false,
        context: AggregateContext::default(),
    }
}

fn a_photo() -> Picture {
    Picture {
        format: Format::Jpeg,
        width: 1_280,
        height: 1_024,
        original_size: 230_000,
        variants: vec![384, 768, 1_280],
    }
}

fn of_the_loom() -> Owner {
    Owner::Project(ProjectLink::from("project_loom"))
}

fn an_upload(owner: Owner) -> AssetEvent {
    AssetEvent::Uploaded {
        owner,
        picture: a_photo(),
        total_size: 400_000,
    }
}

fn uploaded_at(seconds: i64) -> Asset {
    Asset::from_first(&an_upload(of_the_loom()), &stamped(1, clock::at(seconds)))
        .expect("an upload gives birth")
}

#[test]
fn an_asset_is_uploaded_for_its_owner_with_its_picture_and_what_it_keeps() {
    // when: a photo is uploaded into a project
    let decided = Asset::begin(
        AssetCommand::Upload {
            owner: of_the_loom(),
            picture: a_photo(),
            total_size: 400_000,
        },
        &ada(),
    );

    // then: it is uploaded as asked
    assert_eq!(decided, Ok(vec![an_upload(of_the_loom())]));
    // and: the asset is the project's, with its picture and every byte it keeps, from that moment
    let asset = uploaded_at(1_000);
    assert_eq!(asset.owner(), &of_the_loom());
    assert_eq!(asset.picture(), &a_photo());
    assert_eq!(asset.total_size(), 400_000);
    assert_eq!(asset.uploaded_at(), clock::at(1_000));
    assert!(!asset.is_deleted());
}

#[test]
fn a_projects_asset_tells_its_project_with_its_messages_and_an_accounts_does_not() {
    // given: an asset of a project and one of an account
    let projects = uploaded_at(1_000);
    let accounts = Asset::from_first(
        &an_upload(Owner::Account(AccountId::from("account_ada"))),
        &stamped(1, clock::at(1_000)),
    )
    .expect("an upload gives birth");

    // expect: the project's names its project, the account's nothing
    assert_eq!(
        projects.context().get(context::PROJECT),
        Some("project_loom")
    );
    assert!(accounts.context().is_empty());
}

#[test]
fn a_deleted_asset_is_deleted_once_and_takes_nothing_more() {
    // given: an asset
    let mut asset = uploaded_at(1_000);

    // when: it is deleted
    let deleted = asset.decide(AssetCommand::Delete, &ada());
    assert_eq!(deleted, Ok(vec![AssetEvent::Deleted]));
    asset.apply(&AssetEvent::Deleted, &stamped(2, clock::at(2_000)));

    // then: it is deleted from that moment
    assert_eq!(asset.deleted_at(), Some(clock::at(2_000)));
    // and: deleting it again changes nothing
    assert_eq!(asset.decide(AssetCommand::Delete, &ada()), Ok(Vec::new()));
    // and: it is not uploaded anew
    assert_eq!(
        asset.decide(
            AssetCommand::Upload {
                owner: of_the_loom(),
                picture: a_photo(),
                total_size: 1,
            },
            &ada()
        ),
        Err(AssetError::Deleted)
    );
}

#[test]
fn an_asset_is_uploaded_once_and_nothing_is_deleted_before() {
    // expect: an asset not uploaded is not deleted
    assert_eq!(
        Asset::begin(AssetCommand::Delete, &ada()),
        Err(AssetError::NotUploadedYet)
    );
    // and: one uploaded is not uploaded again
    assert_eq!(
        uploaded_at(1_000).decide(
            AssetCommand::Upload {
                owner: of_the_loom(),
                picture: a_photo(),
                total_size: 1,
            },
            &ada()
        ),
        Err(AssetError::AlreadyUploaded)
    );
}

#[test]
fn a_snapshot_stands_for_everything_before_it() {
    // given: an asset uploaded and deleted
    let mut asset = uploaded_at(1_000);
    asset.apply(&AssetEvent::Deleted, &stamped(2, clock::at(2_000)));

    // when: it is snapshotted and read back from the snapshot alone
    let read = Asset::from_first(&asset.snapshot(), &stamped(3, clock::at(3_000)))
        .expect("a snapshot gives birth");

    // then: it is the asset it was
    assert_eq!(read, asset);
}

#[test]
fn each_format_tells_its_content_type() {
    // expect: each format its own content type
    assert_eq!(
        [Format::Jpeg, Format::Png, Format::Gif, Format::Webp].map(Format::content_type),
        ["image/jpeg", "image/png", "image/gif", "image/webp"]
    );
}
