use eventsourcing::Recorded;
use messaging::Message;
use outline_contract::{AttachmentDTO, OutlineEventDTO};
use outline_core::{Attachment, OutlineEvent};

pub fn message_for(happened: &Recorded<OutlineEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn to_dto(attachment: &Attachment) -> AttachmentDTO {
    match attachment {
        Attachment::Scene(scene) => AttachmentDTO::Scene {
            id: scene.to_string(),
        },
        Attachment::Idea(idea) => AttachmentDTO::Idea {
            id: idea.to_string(),
        },
    }
}

fn body(event: &OutlineEvent) -> Option<OutlineEventDTO> {
    Some(match event {
        OutlineEvent::Started { project } => OutlineEventDTO::Started {
            project: project.to_string(),
        },
        OutlineEvent::SectionAdded {
            section,
            under,
            after,
            title,
        } => OutlineEventDTO::SectionAdded {
            section: section.to_string(),
            under: under.map(|under| under.to_string()),
            after: after.map(|after| after.to_string()),
            title: title.to_string(),
        },
        OutlineEvent::SectionRetitled { section, title } => OutlineEventDTO::SectionRetitled {
            section: section.to_string(),
            title: title.to_string(),
        },
        OutlineEvent::SectionMoved {
            section,
            under,
            after,
        } => OutlineEventDTO::SectionMoved {
            section: section.to_string(),
            under: under.map(|under| under.to_string()),
            after: after.map(|after| after.to_string()),
        },
        OutlineEvent::SectionPromoted { section } => OutlineEventDTO::SectionPromoted {
            section: section.to_string(),
        },
        OutlineEvent::SectionDemoted { section } => OutlineEventDTO::SectionDemoted {
            section: section.to_string(),
        },
        OutlineEvent::SectionRemoved { section } => OutlineEventDTO::SectionRemoved {
            section: section.to_string(),
        },
        OutlineEvent::Attached {
            attachment,
            to,
            after,
        } => OutlineEventDTO::Attached {
            attachment: to_dto(attachment),
            to: to.to_string(),
            after: after.as_ref().map(to_dto),
        },
        OutlineEvent::Detached { attachment, from } => OutlineEventDTO::Detached {
            attachment: to_dto(attachment),
            from: from.to_string(),
        },
        OutlineEvent::Discarded => OutlineEventDTO::Discarded,
        OutlineEvent::Snapshotted { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use eventsourcing::{
        AccountId, Agent, AggregateContext, AggregateId, Event, EventMetadata, Version,
    };
    use messaging::{Handler, RoutingKey, Subscription};
    use outline_contract::{
        ATTACHED, DETACHED, EVERY_OUTLINE, SECTION_ADDED, SECTION_DEMOTED, SECTION_MOVED,
        SECTION_PROMOTED, SECTION_REMOVED, SECTION_RETITLED, STARTED,
    };
    use outline_core::{
        Attachment, KIND, OutlineId, PlacedSection, ProjectLink, SectionId, SectionTitle,
    };
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn heard_by(patterns: &[&str], routing: &RoutingKey) -> bool {
        patterns.iter().any(|pattern| {
            Subscription::parse(pattern)
                .expect("a declared pattern")
                .covers(routing)
        })
    }

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn an_outline() -> OutlineId {
        OutlineId::generate(at(1_000))
    }

    fn a_section() -> SectionId {
        SectionId::generate(at(2_000))
    }

    fn titled(what: &str) -> SectionTitle {
        SectionTitle::new(what).expect("the title should be usable")
    }

    fn recorded(id: &OutlineId, event: OutlineEvent) -> Recorded<OutlineEvent> {
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

    fn attaching_to(section: SectionId) -> OutlineEvent {
        OutlineEvent::Attached {
            attachment: Attachment::scene("scene_1"),
            to: section,
            after: None,
        }
    }

    fn an_attachment() -> OutlineEvent {
        attaching_to(a_section())
    }

    fn everything_worth_publishing() -> Vec<(OutlineEvent, &'static str)> {
        vec![
            (
                OutlineEvent::Started {
                    project: ProjectLink::from("project_1"),
                },
                STARTED,
            ),
            (
                OutlineEvent::SectionAdded {
                    section: a_section(),
                    under: None,
                    after: None,
                    title: titled("Part One"),
                },
                SECTION_ADDED,
            ),
            (
                OutlineEvent::SectionRetitled {
                    section: a_section(),
                    title: titled("Part Two"),
                },
                SECTION_RETITLED,
            ),
            (
                OutlineEvent::SectionMoved {
                    section: a_section(),
                    under: None,
                    after: None,
                },
                SECTION_MOVED,
            ),
            (
                OutlineEvent::SectionPromoted {
                    section: a_section(),
                },
                SECTION_PROMOTED,
            ),
            (
                OutlineEvent::SectionDemoted {
                    section: a_section(),
                },
                SECTION_DEMOTED,
            ),
            (
                OutlineEvent::SectionRemoved {
                    section: a_section(),
                },
                SECTION_REMOVED,
            ),
            (an_attachment(), ATTACHED),
            (
                OutlineEvent::Detached {
                    attachment: Attachment::scene("scene_1"),
                    from: a_section(),
                },
                DETACHED,
            ),
        ]
    }

    fn published(id: &OutlineId, event: OutlineEvent) -> Message {
        message_for(&recorded(id, event)).expect("this event should be published")
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        let id = an_outline();

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
    fn the_subscription_the_contract_declares_is_the_one_the_library_derives() {
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_OUTLINE);
        assert!(
            everything_worth_publishing()
                .into_iter()
                .all(|(event, _)| heard_by(
                    &[EVERY_OUTLINE],
                    &eventpublishing::routing_for(KIND, event.name())
                )),
            "a listener asking for every outline must hear all of them"
        );
    }

    #[test]
    fn restructuring_does_not_wake_the_catalog_projector() {
        for (event, _) in everything_worth_publishing() {
            let woken = heard_by(
                <crate::CatalogueOutlinesListener as Handler>::LISTENS_TO,
                &eventpublishing::routing_for(KIND, event.name()),
            );

            assert_eq!(
                woken,
                matches!(
                    event,
                    OutlineEvent::Started { .. } | OutlineEvent::Discarded
                ),
                "which outline a project has can only change when one is started or discarded, so \
                 a move must not cost a projection write"
            );
        }
    }

    #[test]
    fn removing_a_section_wakes_the_attachment_index_because_it_detaches_scenes() {
        let removal = eventpublishing::routing_for(
            KIND,
            OutlineEvent::SectionRemoved {
                section: a_section(),
            }
            .name(),
        );

        assert!(
            heard_by(
                <crate::IndexAttachmentsListener as Handler>::LISTENS_TO,
                &removal
            ),
            "a removed section returns its scenes to the pool, so the index would go stale without \
             it"
        );
    }

    #[test]
    fn an_attachment_carries_the_scene_and_the_section_it_landed_in() {
        let id = an_outline();
        let section = a_section();

        let told = eventpublishing::published_in::<OutlineEventDTO>(&published(
            &id,
            attaching_to(section),
        ))
        .expect("what we wrote must be readable");

        assert_eq!(
            told.event.body,
            OutlineEventDTO::Attached {
                attachment: AttachmentDTO::Scene {
                    id: "scene_1".to_owned(),
                },
                to: section.to_string(),
                after: None,
            }
        );
    }

    #[test]
    fn an_intent_move_carries_only_the_section_because_where_it_lands_is_derived() {
        let id = an_outline();
        let section = a_section();
        let promoted = OutlineEvent::SectionPromoted { section };

        let told = eventpublishing::published_in::<OutlineEventDTO>(&published(&id, promoted))
            .expect("what we wrote must be readable");

        assert_eq!(
            told.event.body,
            OutlineEventDTO::SectionPromoted {
                section: section.to_string(),
            }
        );
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        let id = an_outline();

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
        let id = an_outline();
        let snapshot = OutlineEvent::Snapshotted {
            project: ProjectLink::from("project_1"),
            sections: vec![PlacedSection {
                section: a_section(),
                parent: None,
                title: titled("Part One"),
                attachments: vec![Attachment::scene("scene_1")],
            }],
            discarded: false,
        };

        assert!(
            message_for(&recorded(&id, snapshot)).is_none(),
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }

    #[test]
    fn the_outline_can_be_read_back_out_of_a_message() {
        let id = an_outline();

        assert_eq!(
            eventpublishing::aggregate_in::<OutlineId>(&published(&id, an_attachment()))
                .expect("what we wrote must be readable"),
            id
        );
    }
}

#[cfg(test)]
mod notes {
    use outline_core::{Attachment, SectionId};

    use super::*;

    #[test]
    fn a_note_goes_on_the_wire_tagged_as_an_idea() {
        let told = body(&OutlineEvent::Attached {
            attachment: Attachment::idea("idea_1"),
            to: SectionId::generate(time::OffsetDateTime::UNIX_EPOCH),
            after: None,
        })
        .expect("an attachment is published");

        let OutlineEventDTO::Attached { attachment, .. } = told else {
            panic!("expected an attachment, got {told:?}");
        };
        assert_eq!(
            attachment,
            AttachmentDTO::Idea {
                id: "idea_1".to_owned()
            },
            "a subscriber cannot look the id up to find out what it is, so the tag has to \
             survive the crossing or every note reads as a scene"
        );
    }

    #[test]
    fn a_detachment_goes_on_the_wire_with_the_section_it_left() {
        let left = SectionId::generate(time::OffsetDateTime::UNIX_EPOCH);

        let told = body(&OutlineEvent::Detached {
            attachment: Attachment::idea("idea_1"),
            from: left,
        })
        .expect("a detachment is published");

        let OutlineEventDTO::Detached { from, .. } = told else {
            panic!("expected a detachment, got {told:?}");
        };
        assert_eq!(
            from,
            left.to_string(),
            "a listener keyed by section has nothing else to forget the note by"
        );
    }
}
