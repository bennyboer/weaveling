use eventsourcing::{Agent, EventMetadata};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PublishedEvent<B> {
    pub event: PublishedBody<B>,
    pub aggregate: PublishedAggregate,
    pub agent: PublishedAgent,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PublishedBody<B> {
    pub version: u64,
    #[serde(flatten)]
    pub body: B,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PublishedAggregate {
    pub id: String,
    pub kind: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PublishedAgent {
    pub kind: String,
    pub id: Option<String>,
}

impl PublishedAggregate {
    pub fn of(metadata: &EventMetadata) -> Self {
        Self {
            id: metadata.aggregate.as_str().to_owned(),
            kind: metadata.kind.as_str().to_owned(),
            version: metadata.version.count(),
        }
    }
}

impl PublishedAgent {
    pub fn of(agent: &Agent) -> Self {
        match agent {
            Agent::Anonymous => Self {
                kind: "anonymous".to_owned(),
                id: None,
            },
            Agent::System => Self {
                kind: "system".to_owned(),
                id: None,
            },
            Agent::User(id) => Self {
                kind: "user".to_owned(),
                id: Some(id.to_string()),
            },
        }
    }
}
