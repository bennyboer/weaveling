use eventsourcing::Recorded;
use feedback_contract::{AboutDTO, FeedbackEventDTO, ReadingCopyPlaceDTO};
use feedback_core::{About, FeedbackEvent, ReadingCopyPlace};
use messaging::Message;

/// The message a piece of feedback's event goes out as; a snapshot goes out as none.
pub fn message_for(happened: &Recorded<FeedbackEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &FeedbackEvent) -> Option<FeedbackEventDTO> {
    Some(match event {
        FeedbackEvent::Drafted { about, .. } => FeedbackEventDTO::Drafted {
            about: to_about_dto(about),
        },
        FeedbackEvent::Reworded(_) => FeedbackEventDTO::Reworded,
        FeedbackEvent::Submitted => FeedbackEventDTO::Submitted,
        FeedbackEvent::MarkedDone => FeedbackEventDTO::MarkedDone,
        FeedbackEvent::MarkedNotDone => FeedbackEventDTO::MarkedNotDone,
        FeedbackEvent::Deleted => FeedbackEventDTO::Deleted,
        FeedbackEvent::Snapshotted { .. } => return None,
    })
}

fn to_about_dto(about: &About) -> AboutDTO {
    match about {
        About::ReadingCopy {
            reading_copy,
            place,
        } => AboutDTO::ReadingCopy {
            reading_copy: reading_copy.to_string(),
            place: match place {
                ReadingCopyPlace::Passage {
                    scene,
                    block_path,
                    from,
                    to,
                    quote,
                } => ReadingCopyPlaceDTO::Passage {
                    scene: scene.to_string(),
                    block_path: block_path.indices().to_vec(),
                    from: *from,
                    to: *to,
                    quote: quote.clone(),
                },
                ReadingCopyPlace::Scene(scene) => ReadingCopyPlaceDTO::Scene {
                    scene: scene.to_string(),
                },
                ReadingCopyPlace::Section(section) => ReadingCopyPlaceDTO::Section {
                    section: section.to_string(),
                },
                ReadingCopyPlace::Whole => ReadingCopyPlaceDTO::Whole,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{
        AccountId, Agent, AggregateContext, AggregateId, Event, EventMetadata, Version,
    };
    use feedback_contract::{
        DELETED, DRAFTED, EVERY_FEEDBACK, MARKED_DONE, MARKED_NOT_DONE, REWORDED, SUBMITTED,
    };
    use feedback_core::{
        Comment, CommentBlock, CommentRun, KIND, ProjectLink, ReadingCopyLink, SceneLink,
    };
    use time::OffsetDateTime;

    use super::*;

    const AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    fn saying(text: &str) -> Comment {
        Comment::new(vec![CommentBlock::Paragraph(vec![CommentRun::Text {
            text: text.to_owned(),
            emphasis: false,
            strong: false,
            link: None,
        }])])
        .expect("a plain comment is fine")
    }

    fn on_the_harbour() -> About {
        About::ReadingCopy {
            reading_copy: ReadingCopyLink::from("copy_loom"),
            place: ReadingCopyPlace::Scene(SceneLink::from("scene_harbour")),
        }
    }

    fn recorded(event: FeedbackEvent) -> Recorded<FeedbackEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from("feedback_1"),
                kind: KIND,
                version: Version::of(1),
                agent: Agent::Account(AccountId::from("account_ada")),
                occurred_at: AT,
                is_snapshot: event.is_snapshot(),
                context: AggregateContext::default(),
            },
            event,
        }
    }

    fn published(event: FeedbackEvent) -> Message {
        message_for(&recorded(event)).expect("this event should be published")
    }

    fn everything_worth_publishing() -> Vec<(FeedbackEvent, &'static str)> {
        vec![
            (
                FeedbackEvent::Drafted {
                    project: ProjectLink::from("project_loom"),
                    about: on_the_harbour(),
                    comment: saying("Slow."),
                },
                DRAFTED,
            ),
            (FeedbackEvent::Reworded(saying("Too slow.")), REWORDED),
            (FeedbackEvent::Submitted, SUBMITTED),
            (FeedbackEvent::MarkedDone, MARKED_DONE),
            (FeedbackEvent::MarkedNotDone, MARKED_NOT_DONE),
            (FeedbackEvent::Deleted, DELETED),
        ]
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        // expect: every event lands on the key its contract declares, and the subscription to all of them covers it
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_FEEDBACK);
        for (event, declared) in everything_worth_publishing() {
            assert_eq!(
                eventpublishing::routing_for(KIND, event.name()).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        // expect: every message is named as its event
        for (event, _) in everything_worth_publishing() {
            let expected = event.name().as_str().to_owned();

            assert_eq!(
                published(event).payload["event"]["name"],
                expected,
                "the contract's serde tags and the aggregate's event names must not drift apart"
            );
        }
    }

    #[test]
    fn a_draft_tells_what_it_is_about_and_no_message_tells_what_it_says() {
        // when: every event is published
        for (event, _) in everything_worth_publishing() {
            let message = published(event);

            // then: no message carries the words of the comment
            let written = message.payload.to_string();
            assert!(!written.contains("Slow."), "{written}");
            assert!(!written.contains("Too slow."), "{written}");
        }

        // when: a draft is published
        let told = eventpublishing::published_in::<FeedbackEventDTO>(&published(
            everything_worth_publishing().remove(0).0,
        ))
        .expect("what we wrote must be readable");

        // then: it says what it is about
        assert_eq!(
            told.event.body,
            FeedbackEventDTO::Drafted {
                about: AboutDTO::ReadingCopy {
                    reading_copy: "copy_loom".to_owned(),
                    place: ReadingCopyPlaceDTO::Scene {
                        scene: "scene_harbour".to_owned(),
                    },
                },
            }
        );
    }

    #[test]
    fn a_snapshot_is_not_published_at_all() {
        // given: a snapshot of a piece
        let snapshot = FeedbackEvent::Snapshotted {
            project: ProjectLink::from("project_loom"),
            about: Some(on_the_harbour()),
            comment: Some(saying("Slow.")),
            written_by: Agent::Local,
            written_at: AT,
            submitted_at: None,
            edited: false,
            done: false,
            deleted: false,
        };

        // then: it makes no message
        assert!(
            message_for(&recorded(snapshot)).is_none(),
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }
}
