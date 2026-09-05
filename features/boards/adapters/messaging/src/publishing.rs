use std::sync::Arc;

use async_trait::async_trait;
use boards_contract::{
    BoardEventDTO, EVERY_BOARD, PIECE_PINNED, PIECE_UNPINNED, STARTED, SizeDTO, SpotDTO,
};
use boards_core::{BoardEvent, BoardId, Size, Spot};
use eventpublishing::{
    MessagingEventPublisher, PublishedEvent, UnreadableMessage, message_for as message_carrying,
    published_in,
};
use eventsourcing::{EventPublisher, PublishError, Recorded};
use ids::InvalidId;
use messaging::{Message, Publisher, Subscription};
use thiserror::Error;

pub struct Publishing {
    publishing: MessagingEventPublisher<BoardEvent, BoardEventDTO>,
}

#[derive(Debug, Error)]
pub enum UnreadableBoardEvent {
    #[error(transparent)]
    NotABoardEvent(#[from] UnreadableMessage),
    #[error("this message names something that is not a board")]
    NotABoardId(#[source] InvalidId),
}

pub fn every_event() -> Subscription {
    Subscription::parse(EVERY_BOARD).expect("the board pattern is written at compile time")
}

pub fn when_pinned() -> Subscription {
    Subscription::parse(PIECE_PINNED).expect("a declared routing key holds no wildcards")
}

pub fn when_unpinned() -> Subscription {
    Subscription::parse(PIECE_UNPINNED).expect("a declared routing key holds no wildcards")
}

pub fn when_started() -> Subscription {
    Subscription::parse(STARTED).expect("a declared routing key holds no wildcards")
}

pub fn event_in(message: &Message) -> Result<PublishedEvent<BoardEventDTO>, UnreadableBoardEvent> {
    Ok(published_in(message)?)
}

pub fn board_in(message: &Message) -> Result<BoardId, UnreadableBoardEvent> {
    event_in(message)?
        .aggregate
        .id
        .parse()
        .map_err(UnreadableBoardEvent::NotABoardId)
}

pub fn message_for(happened: &Recorded<BoardEvent>) -> Option<Message> {
    message_carrying(happened, body)
}

impl Publishing {
    pub fn new(publisher: Arc<dyn Publisher>) -> Self {
        Self {
            publishing: MessagingEventPublisher::new(publisher, body),
        }
    }
}

#[async_trait]
impl EventPublisher<BoardEvent> for Publishing {
    async fn publish(&self, happened: &Recorded<BoardEvent>) -> Result<(), PublishError> {
        self.publishing
            .publish(happened)
            .await
            .map_err(PublishError::because)
    }
}

fn body(event: &BoardEvent) -> Option<BoardEventDTO> {
    Some(match event {
        BoardEvent::Started { project } => BoardEventDTO::Started {
            project: project.to_string(),
        },
        BoardEvent::PiecePinned { piece, at, size } => BoardEventDTO::PiecePinned {
            piece: piece.to_string(),
            at: to_spot_dto(*at),
            size: to_size_dto(*size),
        },
        BoardEvent::PieceMoved { piece, to } => BoardEventDTO::PieceMoved {
            piece: piece.to_string(),
            to: to_spot_dto(*to),
        },
        BoardEvent::PieceResized { piece, to } => BoardEventDTO::PieceResized {
            piece: piece.to_string(),
            to: to_size_dto(*to),
        },
        BoardEvent::PieceUnpinned { piece } => BoardEventDTO::PieceUnpinned {
            piece: piece.to_string(),
        },
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
    use boards_contract::{PIECE_MOVED, PIECE_PINNED, PIECE_RESIZED, PIECE_UNPINNED};
    use boards_core::{KIND, PieceLink, PositionedPiece, ProjectLink};
    use eventpublishing::{everything_from, routing_for};
    use eventsourcing::{Agent, AgentId, AggregateId, Event, EventMetadata, Version};
    use serde_json::json;
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
        BoardEvent::PiecePinned {
            piece: PieceLink::from("piece_1"),
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
            (a_pin(), PIECE_PINNED),
            (
                BoardEvent::PieceMoved {
                    piece: PieceLink::from("piece_1"),
                    to: Spot::at(7, 8),
                },
                PIECE_MOVED,
            ),
            (
                BoardEvent::PieceResized {
                    piece: PieceLink::from("piece_1"),
                    to: Size::of(400, 90),
                },
                PIECE_RESIZED,
            ),
            (
                BoardEvent::PieceUnpinned {
                    piece: PieceLink::from("piece_1"),
                },
                PIECE_UNPINNED,
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
                .all(|(event, _)| every_event().covers(&routing_for(KIND, event.name()))),
            "a listener asking for every board must hear all of them"
        );
    }

    #[test]
    fn a_drag_does_not_wake_the_catalog_projector() {
        let listening = when_started();

        assert!(
            listening.covers(&routing_for(
                KIND,
                BoardEvent::Started {
                    project: ProjectLink::from("project_1")
                }
                .name()
            ))
        );
        for pinning in [
            a_pin(),
            BoardEvent::PieceMoved {
                piece: PieceLink::from("piece_1"),
                to: Spot::ORIGIN,
            },
            BoardEvent::PieceUnpinned {
                piece: PieceLink::from("piece_1"),
            },
        ] {
            assert!(
                !listening.covers(&routing_for(KIND, pinning.name())),
                "which board a project has can only change when one is started, so a drop must not cost a projection write"
            );
        }
    }

    #[test]
    fn a_pin_carries_the_piece_and_where_it_landed() {
        let id = a_board();

        let told = event_in(&published(&id, a_pin())).expect("what we wrote must be readable");

        assert_eq!(
            told.event.body,
            BoardEventDTO::PiecePinned {
                piece: "piece_1".to_owned(),
                at: SpotDTO { x: 120, y: -40 },
                size: SizeDTO {
                    width: 400,
                    height: 90,
                },
            }
        );
    }

    #[test]
    fn a_resize_carries_the_piece_and_its_new_extent() {
        let id = a_board();
        let stretched = BoardEvent::PieceResized {
            piece: PieceLink::from("piece_1"),
            to: Size::of(400, 90),
        };

        let told = event_in(&published(&id, stretched)).expect("what we wrote must be readable");

        assert_eq!(
            told.event.body,
            BoardEventDTO::PieceResized {
                piece: "piece_1".to_owned(),
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
            pieces: vec![PositionedPiece {
                piece: PieceLink::from("piece_1"),
                spot: Spot::ORIGIN,
                size: Size::CARD,
            }],
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
            board_in(&published(&id, a_pin())).expect("what we wrote must be readable"),
            id
        );
    }

    #[test]
    fn a_message_about_something_else_entirely_is_refused() {
        let stray = Message::opening(
            routing_for(KIND, a_pin().name()),
            json!({ "nothing": "useful" }),
            at(1_000),
        );

        assert!(matches!(
            event_in(&stray),
            Err(UnreadableBoardEvent::NotABoardEvent(..))
        ));
    }
}
