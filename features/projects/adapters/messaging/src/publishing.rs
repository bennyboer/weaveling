use std::sync::Arc;

use async_trait::async_trait;
use eventpublishing::{
    MessagingEventPublisher, PublishedEvent, UnreadableMessage, message_for as message_carrying,
    published_in,
};
use eventsourcing::{EventPublisher, PublishError, Recorded};
use ids::InvalidId;
use messaging::{Message, Publisher, Subscription};
use projects_contract::{EVERY_PROJECT, ProjectEventDTO};
use projects_core::{ProjectEvent, ProjectId};
use thiserror::Error;

pub struct ProjectEventPublisher {
    publishing: MessagingEventPublisher<ProjectEvent, ProjectEventDTO>,
}

#[derive(Debug, Error)]
pub enum UnreadableProjectEvent {
    #[error(transparent)]
    NotAProjectEvent(#[from] UnreadableMessage),
    #[error("this message names something that is not a project")]
    NotAProjectId(#[source] InvalidId),
}

pub fn every_event() -> Subscription {
    Subscription::parse(EVERY_PROJECT).expect("the project pattern is written at compile time")
}

pub fn event_in(
    message: &Message,
) -> Result<PublishedEvent<ProjectEventDTO>, UnreadableProjectEvent> {
    Ok(published_in(message)?)
}

pub fn project_in(message: &Message) -> Result<ProjectId, UnreadableProjectEvent> {
    event_in(message)?
        .aggregate
        .id
        .parse()
        .map_err(UnreadableProjectEvent::NotAProjectId)
}

pub fn message_for(happened: &Recorded<ProjectEvent>) -> Option<Message> {
    message_carrying(happened, body)
}

impl ProjectEventPublisher {
    pub fn new(publisher: Arc<dyn Publisher>) -> Self {
        Self {
            publishing: MessagingEventPublisher::new(publisher, body),
        }
    }
}

#[async_trait]
impl EventPublisher<ProjectEvent> for ProjectEventPublisher {
    async fn publish(&self, happened: &Recorded<ProjectEvent>) -> Result<(), PublishError> {
        self.publishing
            .publish(happened)
            .await
            .map_err(PublishError::because)
    }
}

fn body(event: &ProjectEvent) -> Option<ProjectEventDTO> {
    Some(match event {
        ProjectEvent::Started(name) => ProjectEventDTO::Started {
            name: name.to_string(),
        },
        ProjectEvent::Renamed(name) => ProjectEventDTO::Renamed {
            name: name.to_string(),
        },
        ProjectEvent::Deleted => ProjectEventDTO::Deleted,
        ProjectEvent::Snapshotted { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use eventpublishing::{everything_from, routing_for};
    use eventsourcing::{Agent, AgentId, AggregateId, Event, EventMetadata, Recorded, Version};
    use messaging::RoutingKey;
    use projects_contract::{DELETED, RENAMED, STARTED};
    use projects_core::{KIND, ProjectName};
    use serde_json::json;
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
                agent: Agent::User(AgentId::from("author-7")),
                occurred_at: at(1_000),
                is_snapshot: event.is_snapshot(),
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
                routing_for(KIND, name).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(&id, event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_subscription_the_contract_declares_is_the_one_the_library_derives() {
        assert_eq!(everything_from(KIND), EVERY_PROJECT);
        assert!(
            everything_worth_publishing()
                .into_iter()
                .all(|(event, _)| { every_event().covers(&routing_for(KIND, event.name())) }),
            "a listener asking for every project must handle all of them"
        );
    }

    #[test]
    fn what_a_project_event_says_comes_from_the_features_own_mapping() {
        let id = a_project();

        let body = event_in(&published(&id, a_start())).expect("what we wrote must be readable");

        assert_eq!(
            body.event.body,
            ProjectEventDTO::Started {
                name: "The Weaver's Apprentice".to_owned(),
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
            project_in(&published(&id, a_start())).expect("what we wrote must be readable"),
            id
        );
    }

    #[test]
    fn a_message_about_something_else_entirely_is_refused() {
        let stray = Message::opening(
            RoutingKey::parse("project.started").expect("a plain key is fine"),
            json!({ "nothing": "useful" }),
            at(1_000),
        );

        assert!(matches!(
            event_in(&stray),
            Err(UnreadableProjectEvent::NotAProjectEvent(..))
        ));
    }
}
