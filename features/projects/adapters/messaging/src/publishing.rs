use eventsourcing::Recorded;
use messaging::Message;
use projects_contract::ProjectEventDTO;
use projects_core::ProjectEvent;

pub fn message_for(happened: &Recorded<ProjectEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &ProjectEvent) -> Option<ProjectEventDTO> {
    Some(match event {
        ProjectEvent::Started(name) => ProjectEventDTO::Started {
            name: name.to_string(),
        },
        ProjectEvent::Renamed(name) => ProjectEventDTO::Renamed {
            name: name.to_string(),
        },
        ProjectEvent::DetailsChanged(details) => ProjectEventDTO::DetailsChanged {
            language: details.language.as_ref().map(ToString::to_string),
            quotation: details.quotation.map(|style| style.to_string()),
            description: details.description.to_string(),
        },
        ProjectEvent::QuotationChanged(style) => ProjectEventDTO::QuotationChanged {
            quotation: style.to_string(),
        },
        ProjectEvent::Deleted => ProjectEventDTO::Deleted,
        ProjectEvent::Snapshotted { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use eventsourcing::{
        AccountId, Agent, AggregateContext, AggregateId, Event, EventMetadata, Recorded, Version,
    };
    use messaging::Subscription;
    use projects_contract::{
        DELETED, DETAILS_CHANGED, EVERY_PROJECT, QUOTATION_CHANGED, RENAMED, STARTED,
    };
    use projects_core::{BookLanguage, ProjectDescription, ProjectDetails, QuotationMarks};
    use projects_core::{KIND, ProjectId, ProjectName};
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn a_project() -> ProjectId {
        ProjectId::generate(at(1_000))
    }

    fn a_name() -> ProjectName {
        ProjectName::new("The Weaver's Apprentice").expect("a plain name is fine")
    }

    fn recorded(id: &ProjectId, event: ProjectEvent) -> Recorded<ProjectEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from(id),
                kind: KIND,
                version: Version::of(1),
                agent: Agent::Account(AccountId::from("author-7")),
                occurred_at: at(1_000),
                is_snapshot: event.is_snapshot(),
                context: AggregateContext::default(),
            },
            event,
        }
    }

    fn a_start() -> ProjectEvent {
        ProjectEvent::Started(a_name())
    }

    fn everything_worth_publishing() -> Vec<(ProjectEvent, &'static str)> {
        vec![
            (a_start(), STARTED),
            (ProjectEvent::Renamed(a_name()), RENAMED),
            (
                ProjectEvent::DetailsChanged(ProjectDetails::default()),
                DETAILS_CHANGED,
            ),
            (
                ProjectEvent::QuotationChanged(QuotationMarks::Curly),
                QUOTATION_CHANGED,
            ),
            (ProjectEvent::Deleted, DELETED),
        ]
    }

    fn published(id: &ProjectId, event: ProjectEvent) -> Message {
        message_for(&recorded(id, event)).expect("this event should be published")
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        let id = a_project();

        for (event, declared) in everything_worth_publishing() {
            let name = event.name();

            assert_eq!(
                eventpublishing::routing_for(KIND, name).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(&id, event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_subscription_the_contract_declares_is_the_one_the_library_derives() {
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_PROJECT);
        assert!(
            everything_worth_publishing().into_iter().all(|(event, _)| {
                Subscription::parse(EVERY_PROJECT)
                    .expect("a declared pattern")
                    .covers(&eventpublishing::routing_for(KIND, event.name()))
            }),
            "a listener asking for every project must handle all of them"
        );
    }

    #[test]
    fn what_a_project_event_says_comes_from_the_features_own_mapping() {
        let id = a_project();

        let body = eventpublishing::published_in::<ProjectEventDTO>(&published(&id, a_start()))
            .expect("what we wrote must be readable");

        assert_eq!(
            body.event.body,
            ProjectEventDTO::Started {
                name: "The Weaver's Apprentice".to_owned(),
            }
        );
    }

    #[test]
    fn changed_details_travel_with_their_language_quotation_and_description() {
        let id = a_project();
        let details = ProjectDetails {
            language: Some(BookLanguage::new("de").expect("a plain tag")),
            quotation: Some(QuotationMarks::LowHigh),
            description: ProjectDescription::new("A weaver's daughter.").expect("plain"),
        };

        let body = eventpublishing::published_in::<ProjectEventDTO>(&published(
            &id,
            ProjectEvent::DetailsChanged(details),
        ))
        .expect("what we wrote must be readable");

        assert_eq!(
            body.event.body,
            ProjectEventDTO::DetailsChanged {
                language: Some("de".to_owned()),
                quotation: Some("low-high".to_owned()),
                description: "A weaver's daughter.".to_owned(),
            }
        );
    }

    #[test]
    fn a_changed_quotation_style_travels_by_its_name() {
        let id = a_project();

        let body = eventpublishing::published_in::<ProjectEventDTO>(&published(
            &id,
            ProjectEvent::QuotationChanged(QuotationMarks::PointingOut),
        ))
        .expect("what we wrote must be readable");

        assert_eq!(
            body.event.body,
            ProjectEventDTO::QuotationChanged {
                quotation: "pointing-out".to_owned(),
            }
        );
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        let id = a_project();

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
        let id = a_project();
        let snapshot = ProjectEvent::Snapshotted {
            name: a_name(),
            details: ProjectDetails::default(),
            created_at: at(1_000),
            updated_at: at(2_000),
            deleted: false,
        };

        assert!(
            message_for(&recorded(&id, snapshot)).is_none(),
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }

    #[test]
    fn the_project_can_be_read_back_out_of_a_message() {
        let id = a_project();

        assert_eq!(
            eventpublishing::aggregate_in::<ProjectId>(&published(&id, a_start()))
                .expect("what we wrote must be readable"),
            id
        );
    }
}
