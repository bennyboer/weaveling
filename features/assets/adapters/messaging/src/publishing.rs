use assets_contract::{AssetEventDTO, OwnerDTO, format};
use assets_core::{AssetEvent, Format, Owner};
use eventsourcing::Recorded;
use messaging::Message;

pub fn message_for(happened: &Recorded<AssetEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &AssetEvent) -> Option<AssetEventDTO> {
    Some(match event {
        AssetEvent::Uploaded {
            owner,
            picture,
            total_size,
        } => AssetEventDTO::Uploaded {
            owner: owner_told(owner),
            format: format_told(picture.format).to_owned(),
            width: picture.width,
            height: picture.height,
            original_size: picture.original_size,
            total_size: *total_size,
        },
        AssetEvent::Deleted => AssetEventDTO::Deleted,
        AssetEvent::Snapshotted { .. } => return None,
    })
}

fn owner_told(owner: &Owner) -> OwnerDTO {
    match owner {
        Owner::Project(project) => OwnerDTO::Project(project.to_string()),
        Owner::Account(account) => OwnerDTO::Account(account.to_string()),
    }
}

fn format_told(told: Format) -> &'static str {
    match told {
        Format::Jpeg => format::JPEG,
        Format::Png => format::PNG,
        Format::Gif => format::GIF,
        Format::Webp => format::WEBP,
    }
}

#[cfg(test)]
mod tests {
    use assets_contract::{DELETED, EVERY_ASSET, UPLOADED};
    use assets_core::{AssetId, KIND, Picture, ProjectLink};
    use eventsourcing::{
        AccountId, Agent, AggregateContext, AggregateId, Event, EventMetadata, Version,
    };
    use messaging::Subscription;

    use super::*;

    fn an_asset() -> AssetId {
        AssetId::generate(clock::at(1_000))
    }

    fn a_picture() -> Picture {
        Picture {
            format: Format::Png,
            width: 1200,
            height: 800,
            original_size: 300_000,
            variants: vec![384, 768],
        }
    }

    fn recorded(id: &AssetId, event: AssetEvent) -> Recorded<AssetEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from(id),
                kind: KIND,
                version: Version::of(1),
                agent: Agent::Account(AccountId::from("account_ada")),
                occurred_at: clock::at(1_000),
                is_snapshot: event.is_snapshot(),
                context: AggregateContext::default(),
            },
            event,
        }
    }

    fn an_upload_into(owner: Owner) -> AssetEvent {
        AssetEvent::Uploaded {
            owner,
            picture: a_picture(),
            total_size: 120_000,
        }
    }

    fn everything_worth_publishing() -> Vec<(AssetEvent, &'static str)> {
        vec![
            (
                an_upload_into(Owner::Project(ProjectLink::from("project_1"))),
                UPLOADED,
            ),
            (AssetEvent::Deleted, DELETED),
        ]
    }

    fn published(id: &AssetId, event: AssetEvent) -> Message {
        message_for(&recorded(id, event)).expect("this event should be published")
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        // given: an asset
        let id = an_asset();

        // then: every event is published on the key the contract declares
        for (event, declared) in everything_worth_publishing() {
            assert_eq!(
                eventpublishing::routing_for(KIND, event.name()).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(&id, event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_subscription_the_contract_declares_covers_every_event() {
        // expect: the contract declares the subscription the library derives
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_ASSET);
        // and: it covers every published event
        assert!(
            everything_worth_publishing().into_iter().all(|(event, _)| {
                Subscription::parse(EVERY_ASSET)
                    .expect("a declared pattern")
                    .covers(&eventpublishing::routing_for(KIND, event.name()))
            }),
            "a listener asking for every asset must hear all of them"
        );
    }

    #[test]
    fn an_upload_says_whose_it_is_and_what_it_takes_but_not_its_variants() {
        // given: an asset
        let id = an_asset();

        // when: its upload into a project and one of an account's own are published
        let into_a_project = eventpublishing::published_in::<AssetEventDTO>(&published(
            &id,
            an_upload_into(Owner::Project(ProjectLink::from("project_1"))),
        ))
        .expect("what we wrote must be readable");
        let an_accounts_own = eventpublishing::published_in::<AssetEventDTO>(&published(
            &id,
            an_upload_into(Owner::Account(AccountId::from("account_ada"))),
        ))
        .expect("what we wrote must be readable");

        // then: each says its owner, its picture and what it takes
        assert_eq!(
            into_a_project.event.body,
            AssetEventDTO::Uploaded {
                owner: OwnerDTO::Project("project_1".to_owned()),
                format: "png".to_owned(),
                width: 1200,
                height: 800,
                original_size: 300_000,
                total_size: 120_000,
            }
        );
        assert!(matches!(
            an_accounts_own.event.body,
            AssetEventDTO::Uploaded { owner: OwnerDTO::Account(account), .. } if account == "account_ada"
        ));
    }

    #[test]
    fn every_format_is_told_by_the_name_the_contract_declares() {
        // expect: each format by the contract's name for it
        assert_eq!(format_told(Format::Jpeg), "jpeg");
        assert_eq!(format_told(Format::Png), "png");
        assert_eq!(format_told(Format::Gif), "gif");
        assert_eq!(format_told(Format::Webp), "webp");
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        // given: an asset
        let id = an_asset();

        // then: every event goes out under its own name
        for (event, _) in everything_worth_publishing() {
            let expected = event.name().as_str().to_owned();

            assert_eq!(
                published(&id, event).payload["event"]["name"],
                expected,
                "the contract's serde tags and the aggregate's event names must not drift apart"
            );
        }
    }

    #[test]
    fn a_snapshot_is_not_published_at_all() {
        // given: a snapshot of an asset
        let snapshot = AssetEvent::Snapshotted {
            owner: Owner::Project(ProjectLink::from("project_1")),
            picture: a_picture(),
            total_size: 120_000,
            uploaded_at: clock::at(1_000),
            deleted_at: None,
        };

        // then: it is not published
        assert!(
            message_for(&recorded(&an_asset(), snapshot)).is_none(),
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }

    #[test]
    fn the_asset_can_be_read_back_out_of_a_message() {
        // given: an asset
        let id = an_asset();

        // then: the asset is read back out of its published message
        assert_eq!(
            eventpublishing::aggregate_in::<AssetId>(&published(&id, AssetEvent::Deleted))
                .expect("what we wrote must be readable"),
            id
        );
    }
}
