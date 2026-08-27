use thiserror::Error;
use time::OffsetDateTime;

use crate::agent::Agent;
use crate::aggregate::{Aggregate, AggregateId, AggregateType};
use crate::event::{Event, EventName, Recorded};
use crate::metadata::EventMetadata;
use crate::version::Version;

pub const SAMPLE: AggregateType = AggregateType::of("sample");
pub const CREATED: EventName = EventName::of("CREATED");
pub const TITLE_UPDATED: EventName = EventName::of("TITLE_UPDATED");
pub const DESCRIPTION_UPDATED: EventName = EventName::of("DESCRIPTION_UPDATED");
pub const DELETED: EventName = EventName::of("DELETED");
pub const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

pub enum SampleCommand {
    Create { title: String, description: String },
    UpdateTitle(String),
    UpdateDescription(String),
    Rewrite { title: String, description: String },
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleEvent {
    Created {
        title: String,
        description: String,
    },
    TitleUpdated(String),
    DescriptionUpdated(String),
    Deleted,
    Snapshotted {
        title: String,
        description: String,
        deleted: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub title: String,
    pub description: String,
    pub deleted: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SampleError {
    #[error("a sample must be created before anything else can happen to it")]
    NotCreatedYet,
    #[error("a sample cannot be created twice")]
    AlreadyCreated,
    #[error("a deleted sample accepts no commands")]
    Deleted,
}

impl Event for SampleEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Created { .. } => CREATED,
            Self::TitleUpdated(_) => TITLE_UPDATED,
            Self::DescriptionUpdated(_) => DESCRIPTION_UPDATED,
            Self::Deleted => DELETED,
            Self::Snapshotted { .. } => SNAPSHOTTED,
        }
    }

    fn version(&self) -> Version {
        Version::ZERO
    }

    fn is_snapshot(&self) -> bool {
        matches!(self, Self::Snapshotted { .. })
    }
}

impl Aggregate for Sample {
    type Command = SampleCommand;
    type Event = SampleEvent;
    type Error = SampleError;

    const KIND: AggregateType = SAMPLE;

    fn begin(command: SampleCommand, _agent: &Agent) -> Result<Vec<SampleEvent>, SampleError> {
        match command {
            SampleCommand::Create { title, description } => {
                Ok(vec![SampleEvent::Created { title, description }])
            }
            _ => Err(SampleError::NotCreatedYet),
        }
    }

    fn born(event: &SampleEvent, _metadata: &EventMetadata) -> Option<Self> {
        match event {
            SampleEvent::Created { title, description } => Some(Self {
                title: title.clone(),
                description: description.clone(),
                deleted: false,
            }),
            SampleEvent::Snapshotted {
                title,
                description,
                deleted,
            } => Some(Self {
                title: title.clone(),
                description: description.clone(),
                deleted: *deleted,
            }),
            _ => None,
        }
    }

    fn decide(
        &self,
        command: SampleCommand,
        _agent: &Agent,
    ) -> Result<Vec<SampleEvent>, SampleError> {
        if self.deleted {
            return Err(SampleError::Deleted);
        }

        match command {
            SampleCommand::Create { .. } => Err(SampleError::AlreadyCreated),
            SampleCommand::UpdateTitle(title) => Ok(vec![SampleEvent::TitleUpdated(title)]),
            SampleCommand::UpdateDescription(description) => {
                Ok(vec![SampleEvent::DescriptionUpdated(description)])
            }
            SampleCommand::Rewrite { title, description } => Ok(vec![
                SampleEvent::TitleUpdated(title),
                SampleEvent::DescriptionUpdated(description),
            ]),
            SampleCommand::Delete => Ok(vec![SampleEvent::Deleted]),
        }
    }

    fn absorb(&mut self, event: &SampleEvent, _metadata: &EventMetadata) {
        match event {
            SampleEvent::Created { .. } => {}
            SampleEvent::TitleUpdated(title) => self.title = title.clone(),
            SampleEvent::DescriptionUpdated(description) => self.description = description.clone(),
            SampleEvent::Deleted => self.deleted = true,
            SampleEvent::Snapshotted {
                title,
                description,
                deleted,
            } => {
                self.title = title.clone();
                self.description = description.clone();
                self.deleted = *deleted;
            }
        }
    }

    fn snapshot(&self) -> SampleEvent {
        SampleEvent::Snapshotted {
            title: self.title.clone(),
            description: self.description.clone(),
            deleted: self.deleted,
        }
    }
}

pub fn a_sample() -> AggregateId {
    AggregateId::from("sample_1")
}

pub fn stamped(aggregate: &AggregateId, version: u64) -> EventMetadata {
    EventMetadata {
        aggregate: aggregate.clone(),
        kind: SAMPLE,
        version: Version::of(version),
        agent: Agent::System,
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        is_snapshot: false,
    }
}

pub fn recorded(aggregate: &AggregateId, events: Vec<SampleEvent>) -> Vec<Recorded<SampleEvent>> {
    events
        .into_iter()
        .enumerate()
        .map(|(counted, event)| {
            let mut metadata = stamped(aggregate, counted as u64 + 1);
            metadata.is_snapshot = event.is_snapshot();

            Recorded { event, metadata }
        })
        .collect()
}

pub fn replay(stream: &[Recorded<SampleEvent>]) -> Option<Sample> {
    let (first, rest) = stream.split_first()?;
    let mut sample = Sample::born(&first.event, &first.metadata)?;

    for entry in rest {
        sample.absorb(&entry.event, &entry.metadata);
    }

    Some(sample)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_created_sample() -> Sample {
        replay(&recorded(
            &a_sample(),
            vec![SampleEvent::Created {
                title: "The Loom".to_owned(),
                description: "A silent machine.".to_owned(),
            }],
        ))
        .expect("the creation event gives birth")
    }

    #[test]
    fn nothing_becomes_something_only_through_a_creation_command() {
        let born = Sample::begin(
            SampleCommand::Create {
                title: "The Loom".to_owned(),
                description: "A silent machine.".to_owned(),
            },
            &Agent::System,
        );

        assert_eq!(
            born,
            Ok(vec![SampleEvent::Created {
                title: "The Loom".to_owned(),
                description: "A silent machine.".to_owned(),
            }])
        );
    }

    #[test]
    fn a_command_other_than_creation_cannot_start_a_stream() {
        let refused = Sample::begin(
            SampleCommand::UpdateTitle("The Silent Loom".to_owned()),
            &Agent::System,
        );

        assert_eq!(refused, Err(SampleError::NotCreatedYet));
    }

    #[test]
    fn only_a_creation_or_snapshot_event_gives_birth() {
        let metadata = stamped(&a_sample(), 0);

        assert!(Sample::born(&SampleEvent::Deleted, &metadata).is_none());
        assert!(
            Sample::born(
                &SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
                &metadata
            )
            .is_none()
        );
    }

    #[test]
    fn a_stream_replays_into_the_state_it_describes() {
        let sample = replay(&recorded(
            &a_sample(),
            vec![
                SampleEvent::Created {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                },
                SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
            ],
        ))
        .expect("the stream begins with a creation event");

        assert_eq!(sample.title, "The Silent Loom");
        assert_eq!(sample.description, "A silent machine.");
        assert!(!sample.deleted);
    }

    #[test]
    fn a_stream_that_does_not_begin_with_a_creation_event_replays_into_nothing() {
        let orphaned = recorded(
            &a_sample(),
            vec![SampleEvent::TitleUpdated("Adrift".to_owned())],
        );

        assert!(replay(&orphaned).is_none());
    }

    #[test]
    fn one_command_may_emit_several_events() {
        let decided = a_created_sample().decide(
            SampleCommand::Rewrite {
                title: "The Silent Loom".to_owned(),
                description: "It remembers.".to_owned(),
            },
            &Agent::System,
        );

        assert_eq!(
            decided,
            Ok(vec![
                SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
                SampleEvent::DescriptionUpdated("It remembers.".to_owned()),
            ])
        );
    }

    #[test]
    fn a_description_changes_without_disturbing_the_title() {
        let mut sample = a_created_sample();
        let decided = sample
            .decide(
                SampleCommand::UpdateDescription("It remembers.".to_owned()),
                &Agent::System,
            )
            .expect("a live sample accepts a new description");

        for event in &decided {
            sample.absorb(event, &stamped(&a_sample(), 1));
        }

        assert_eq!(sample.description, "It remembers.");
        assert_eq!(sample.title, "The Loom");
    }

    #[test]
    fn what_exists_cannot_be_created_again() {
        let refused = a_created_sample().decide(
            SampleCommand::Create {
                title: "Another".to_owned(),
                description: "Entirely".to_owned(),
            },
            &Agent::System,
        );

        assert_eq!(refused, Err(SampleError::AlreadyCreated));
    }

    #[test]
    fn a_deleted_aggregate_refuses_every_command() {
        let mut sample = a_created_sample();
        sample.absorb(&SampleEvent::Deleted, &stamped(&a_sample(), 1));

        assert_eq!(
            sample.decide(
                SampleCommand::UpdateTitle("Too late".to_owned()),
                &Agent::System
            ),
            Err(SampleError::Deleted)
        );
        assert_eq!(
            sample.decide(SampleCommand::Delete, &Agent::System),
            Err(SampleError::Deleted)
        );
    }

    #[test]
    fn a_snapshot_event_declares_itself_a_snapshot() {
        assert!(a_created_sample().snapshot().is_snapshot());
        assert_eq!(a_created_sample().snapshot().name(), SNAPSHOTTED);
    }

    #[test]
    fn a_snapshot_replays_into_exactly_the_state_it_came_from() {
        let mut sample = a_created_sample();
        sample.absorb(
            &SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
            &stamped(&a_sample(), 1),
        );
        sample.absorb(&SampleEvent::Deleted, &stamped(&a_sample(), 2));

        let snapshot = sample.snapshot();
        let restored =
            Sample::born(&snapshot, &stamped(&a_sample(), 3)).expect("a snapshot gives birth");

        assert_eq!(restored, sample);
    }

    #[test]
    fn a_snapshot_found_mid_stream_replaces_whatever_came_before() {
        let mut sample = a_created_sample();
        sample.absorb(
            &SampleEvent::Snapshotted {
                title: "Elsewhere".to_owned(),
                description: "Entirely".to_owned(),
                deleted: true,
            },
            &stamped(&a_sample(), 1),
        );

        assert_eq!(sample.title, "Elsewhere");
        assert_eq!(sample.description, "Entirely");
        assert!(sample.deleted);
    }

    #[test]
    fn snapshots_are_taken_by_default() {
        assert_eq!(a_created_sample().snapshot_after(), Some(100));
    }

    #[test]
    fn an_event_knows_its_own_name() {
        assert_eq!(SampleEvent::Deleted.name(), DELETED);
        assert_eq!(
            SampleEvent::TitleUpdated("x".to_owned()).name(),
            TITLE_UPDATED
        );
    }

    #[test]
    fn an_ordinary_event_is_not_a_snapshot() {
        assert!(!SampleEvent::Deleted.is_snapshot());
    }

    #[test]
    fn a_recorded_stream_marks_its_snapshots_in_the_metadata() {
        let stream = recorded(
            &a_sample(),
            vec![
                SampleEvent::Created {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                },
                SampleEvent::Snapshotted {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                    deleted: false,
                },
            ],
        );

        assert!(!stream[0].metadata.is_snapshot);
        assert!(stream[1].metadata.is_snapshot);
    }
}
