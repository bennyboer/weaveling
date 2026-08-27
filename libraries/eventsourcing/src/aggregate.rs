use std::fmt::{self, Display, Formatter};

use crate::agent::Agent;
use crate::event::Event;
use crate::metadata::EventMetadata;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AggregateId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AggregateType(&'static str);

pub trait Aggregate: Sized {
    type Command;
    type Event: Event;
    type Error: std::error::Error;

    const KIND: AggregateType;

    fn begin(command: Self::Command, agent: &Agent) -> Result<Vec<Self::Event>, Self::Error>;

    fn born(event: &Self::Event, metadata: &EventMetadata) -> Option<Self>;

    fn decide(
        &self,
        command: Self::Command,
        agent: &Agent,
    ) -> Result<Vec<Self::Event>, Self::Error>;

    fn absorb(&mut self, event: &Self::Event, metadata: &EventMetadata);

    fn snapshot_after(&self) -> Option<u32> {
        Some(100)
    }
}

impl AggregateId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AggregateId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for AggregateId {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for AggregateId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl AggregateType {
    pub const fn of(named: &'static str) -> Self {
        Self(named)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl Display for AggregateType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use thiserror::Error;
    use time::OffsetDateTime;

    use super::*;
    use crate::event::{EventName, Recorded};
    use crate::version::Version;

    const SAMPLE: AggregateType = AggregateType::of("sample");
    const CREATED: EventName = EventName::of("CREATED");
    const TITLE_UPDATED: EventName = EventName::of("TITLE_UPDATED");
    const DESCRIPTION_UPDATED: EventName = EventName::of("DESCRIPTION_UPDATED");
    const DELETED: EventName = EventName::of("DELETED");

    enum SampleCommand {
        Create { title: String, description: String },
        UpdateTitle(String),
        UpdateDescription(String),
        Rewrite { title: String, description: String },
        Delete,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum SampleEvent {
        Created { title: String, description: String },
        TitleUpdated(String),
        DescriptionUpdated(String),
        Deleted,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Sample {
        title: String,
        description: String,
        deleted: bool,
    }

    #[derive(Debug, Error, PartialEq, Eq)]
    enum SampleError {
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
            }
        }

        fn version(&self) -> Version {
            Version::ZERO
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
                SampleEvent::DescriptionUpdated(description) => {
                    self.description = description.clone()
                }
                SampleEvent::Deleted => self.deleted = true,
            }
        }
    }

    fn stamped(version: u64) -> EventMetadata {
        EventMetadata {
            aggregate: AggregateId::from("sample_1"),
            kind: SAMPLE,
            version: Version::of(version),
            agent: Agent::System,
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            is_snapshot: false,
        }
    }

    fn recorded(events: Vec<SampleEvent>) -> Vec<Recorded<SampleEvent>> {
        events
            .into_iter()
            .enumerate()
            .map(|(counted, event)| Recorded {
                event,
                metadata: stamped(counted as u64),
            })
            .collect()
    }

    fn replay(stream: &[Recorded<SampleEvent>]) -> Option<Sample> {
        let (first, rest) = stream.split_first()?;
        let mut sample = Sample::born(&first.event, &first.metadata)?;

        for entry in rest {
            sample.absorb(&entry.event, &entry.metadata);
        }

        Some(sample)
    }

    fn a_created_sample() -> Sample {
        replay(&recorded(vec![SampleEvent::Created {
            title: "The Loom".to_owned(),
            description: "A silent machine.".to_owned(),
        }]))
        .expect("the creation event gives birth")
    }

    #[test]
    fn an_aggregate_type_is_nameable_at_compile_time() {
        assert_eq!(SAMPLE.as_str(), "sample");
        assert_eq!(SAMPLE.to_string(), "sample");
    }

    #[test]
    fn two_kinds_of_aggregate_are_not_the_same() {
        assert_ne!(SAMPLE, AggregateType::of("board"));
    }

    #[test]
    fn an_aggregate_id_keeps_whatever_the_feature_gave_it() {
        let id = AggregateId::from("piece_019a4f2b");

        assert_eq!(id.as_str(), "piece_019a4f2b");
        assert_eq!(id.to_string(), "piece_019a4f2b");
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
    fn only_a_creation_event_gives_birth() {
        let metadata = stamped(0);

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
        let sample = replay(&recorded(vec![
            SampleEvent::Created {
                title: "The Loom".to_owned(),
                description: "A silent machine.".to_owned(),
            },
            SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
        ]))
        .expect("the stream begins with a creation event");

        assert_eq!(sample.title, "The Silent Loom");
        assert_eq!(sample.description, "A silent machine.");
        assert!(!sample.deleted);
    }

    #[test]
    fn a_stream_that_does_not_begin_with_a_creation_event_replays_into_nothing() {
        let orphaned = recorded(vec![SampleEvent::TitleUpdated("Adrift".to_owned())]);

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
            sample.absorb(event, &stamped(1));
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
        sample.absorb(&SampleEvent::Deleted, &stamped(1));

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
}
