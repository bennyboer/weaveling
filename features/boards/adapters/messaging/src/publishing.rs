use boards_contract::{BoardEventDTO, SizeDTO, SpotDTO};
use boards_core::{BoardEvent, Size, Spot};
use eventpublishing::message_for as message_carrying;
use eventsourcing::Recorded;
use messaging::Message;

pub fn message_for(happened: &Recorded<BoardEvent>) -> Option<Message> {
    message_carrying(happened, body)
}

fn body(event: &BoardEvent) -> Option<BoardEventDTO> {
    Some(match event {
        BoardEvent::Started { project } => BoardEventDTO::Started {
            project: project.to_string(),
        },
        BoardEvent::IdeaPinned { idea, at, size } => BoardEventDTO::IdeaPinned {
            idea: idea.to_string(),
            at: to_spot_dto(*at),
            size: to_size_dto(*size),
        },
        BoardEvent::IdeaMoved { idea, to } => BoardEventDTO::IdeaMoved {
            idea: idea.to_string(),
            to: to_spot_dto(*to),
        },
        BoardEvent::IdeaResized { idea, to } => BoardEventDTO::IdeaResized {
            idea: idea.to_string(),
            to: to_size_dto(*to),
        },
        BoardEvent::IdeaRaised { idea } => BoardEventDTO::IdeaRaised {
            idea: idea.to_string(),
        },
        BoardEvent::IdeaUnpinned { idea } => BoardEventDTO::IdeaUnpinned {
            idea: idea.to_string(),
        },
        BoardEvent::Discarded => BoardEventDTO::Discarded,
        BoardEvent::Snapshotted { .. } => return None,
    })
}

fn to_spot_dto(at: Spot) -> SpotDTO {
    SpotDTO { x: at.x, y: at.y }
}

fn to_size_dto(size: Size) -> SizeDTO {
    SizeDTO {
        width: size.width,
        height: size.height,
    }
}

#[cfg(test)]
mod tests {
    use boards_contract::{
        EVERY_BOARD, IDEA_MOVED, IDEA_PINNED, IDEA_RAISED, IDEA_RESIZED, IDEA_UNPINNED, STARTED,
    };
    use boards_core::{BoardId, IdeaLink, KIND, PositionedIdea, ProjectLink};
    use eventpublishing::{aggregate_in, everything_from, published_in, routing_for};
    use eventsourcing::{Agent, AgentId, AggregateId, Event, EventMetadata, Version};
    use messaging::{Handler, RoutingKey, Subscription};
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn a_board() -> BoardId {
        BoardId::generate(at(1_000))
    }

    fn recorded(id: &BoardId, event: BoardEvent) -> Recorded<BoardEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from(id),
                kind: KIND,
                version: Version::of(1),
                agent: Agent::User(AgentId::from("author-7")),
                occurred_at: at(1_000),
                is_snapshot: event.is_snapshot(),
            },
            event,
        }
    }

    fn a_pin() -> BoardEvent {
        BoardEvent::IdeaPinned {
            idea: IdeaLink::from("idea_1"),
            at: Spot::at(120, -40),
            size: Size::of(400, 90),
        }
    }

    fn everything_worth_publishing() -> Vec<(BoardEvent, &'static str)> {
        vec![
            (
                BoardEvent::Started {
                    project: ProjectLink::from("project_1"),
                },
                STARTED,
            ),
            (a_pin(), IDEA_PINNED),
            (
                BoardEvent::IdeaMoved {
                    idea: IdeaLink::from("idea_1"),
                    to: Spot::at(7, 8),
                },
                IDEA_MOVED,
            ),
            (
                BoardEvent::IdeaResized {
                    idea: IdeaLink::from("idea_1"),
                    to: Size::of(400, 90),
                },
                IDEA_RESIZED,
            ),
            (
                BoardEvent::IdeaRaised {
                    idea: IdeaLink::from("idea_1"),
                },
                IDEA_RAISED,
            ),
            (
                BoardEvent::IdeaUnpinned {
                    idea: IdeaLink::from("idea_1"),
                },
                IDEA_UNPINNED,
            ),
        ]
    }

    fn published(id: &BoardId, event: BoardEvent) -> Message {
        message_for(&recorded(id, event)).expect("this event should be published")
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        let id = a_board();

        for (event, declared) in everything_worth_publishing() {
            assert_eq!(
                routing_for(KIND, event.name()).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(&id, event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_subscription_the_contract_declares_is_the_one_the_library_derives() {
        assert_eq!(everything_from(KIND), EVERY_BOARD);
        assert!(
            everything_worth_publishing()
                .into_iter()
                .all(|(event, _)| Subscription::parse(EVERY_BOARD)
                    .expect("a declared pattern")
                    .covers(&routing_for(KIND, event.name()))),
            "a listener asking for every board must hear all of them"
        );
    }

    #[test]
    fn a_drag_does_not_wake_the_catalog_projector() {
        let listening = |routing: &RoutingKey| {
            <crate::CatalogueBoardsListener as Handler>::LISTENS_TO
                .iter()
                .any(|pattern| {
                    Subscription::parse(pattern)
                        .expect("a declared pattern")
                        .covers(routing)
                })
        };

        assert!(listening(&routing_for(
            KIND,
            BoardEvent::Started {
                project: ProjectLink::from("project_1")
            }
            .name()
        )));
        for pinning in [
            a_pin(),
            BoardEvent::IdeaMoved {
                idea: IdeaLink::from("idea_1"),
                to: Spot::ORIGIN,
            },
            BoardEvent::IdeaUnpinned {
                idea: IdeaLink::from("idea_1"),
            },
        ] {
            assert!(
                !listening(&routing_for(KIND, pinning.name())),
                "which board a project has can only change when one is started, so a drop must not \
                 cost a projection write"
            );
        }
    }

    #[test]
    fn a_pin_carries_the_idea_and_where_it_landed() {
        let id = a_board();

        let told = published_in::<BoardEventDTO>(&published(&id, a_pin()))
            .expect("what we wrote must be readable");

        assert_eq!(
            told.event.body,
            BoardEventDTO::IdeaPinned {
                idea: "idea_1".to_owned(),
                at: SpotDTO { x: 120, y: -40 },
                size: SizeDTO {
                    width: 400,
                    height: 90,
                },
            }
        );
    }

    #[test]
    fn a_resize_carries_the_idea_and_its_new_extent() {
        let id = a_board();
        let stretched = BoardEvent::IdeaResized {
            idea: IdeaLink::from("idea_1"),
            to: Size::of(400, 90),
        };

        let told = published_in::<BoardEventDTO>(&published(&id, stretched))
            .expect("what we wrote must be readable");

        assert_eq!(
            told.event.body,
            BoardEventDTO::IdeaResized {
                idea: "idea_1".to_owned(),
                to: SizeDTO {
                    width: 400,
                    height: 90,
                },
            }
        );
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        let id = a_board();

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
        let id = a_board();
        let snapshot = BoardEvent::Snapshotted {
            project: ProjectLink::from("project_1"),
            ideas: vec![PositionedIdea {
                idea: IdeaLink::from("idea_1"),
                spot: Spot::ORIGIN,
                size: Size::CARD,
            }],
            discarded: false,
        };

        assert!(
            message_for(&recorded(&id, snapshot)).is_none(),
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }

    #[test]
    fn the_board_can_be_read_back_out_of_a_message() {
        let id = a_board();

        assert_eq!(
            aggregate_in::<BoardId>(&published(&id, a_pin()))
                .expect("what we wrote must be readable"),
            id
        );
    }
}
