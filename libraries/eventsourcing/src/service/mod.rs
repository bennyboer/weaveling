use std::sync::Arc;

use clock::Clock;
use thiserror::Error;

use crate::agent::Agent;
use crate::aggregate::{Aggregate, AggregateId, AggregateType};
use crate::event::{Event, Recorded};
use crate::metadata::EventMetadata;
use crate::patch::Patcher;
use crate::store::{EventStore, StoreError};
use crate::version::Version;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ServiceError<E> {
    #[error("there is no {kind} with id {aggregate}")]
    NotFound {
        aggregate: AggregateId,
        kind: AggregateType,
    },
    #[error("the stream for {kind} {aggregate} does not begin with an event that creates it")]
    Unusable {
        aggregate: AggregateId,
        kind: AggregateType,
    },
    #[error(
        "the snapshot for {kind} {aggregate} does not declare itself a snapshot, so it could never be found again"
    )]
    Unmarked {
        aggregate: AggregateId,
        kind: AggregateType,
    },
    #[error(transparent)]
    Refused(E),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub struct EventSourcingService<A: Aggregate> {
    store: Arc<dyn EventStore<A::Event>>,
    patcher: Patcher<A::Event>,
    clock: Arc<dyn Clock>,
}

type Outcome<A> = Result<Version, ServiceError<<A as Aggregate>::Error>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing<A> {
    pub state: A,
    pub version: Version,
}

impl<A> EventSourcingService<A>
where
    A: Aggregate,
    A::Event: Clone + Send + Sync,
{
    pub fn new(store: Arc<dyn EventStore<A::Event>>, clock: Arc<dyn Clock>) -> Self {
        Self {
            store,
            patcher: Patcher::holding(A::patches()),
            clock,
        }
    }

    pub async fn latest(
        &self,
        aggregate: &AggregateId,
    ) -> Result<Standing<A>, ServiceError<A::Error>> {
        self.standing(aggregate, None)
            .await?
            .ok_or_else(|| self.nothing_there(aggregate))
    }

    pub async fn as_of(
        &self,
        aggregate: &AggregateId,
        version: Version,
    ) -> Result<Standing<A>, ServiceError<A::Error>> {
        self.standing(aggregate, Some(version))
            .await?
            .ok_or_else(|| self.nothing_there(aggregate))
    }

    pub async fn begin(
        &self,
        aggregate: &AggregateId,
        command: A::Command,
        agent: &Agent,
    ) -> Outcome<A> {
        self.carry_out(aggregate, None, Version::ZERO, command, agent)
            .await
    }

    pub async fn execute(
        &self,
        aggregate: &AggregateId,
        command: A::Command,
        agent: &Agent,
    ) -> Outcome<A> {
        match self.standing(aggregate, None).await? {
            None => Err(self.nothing_there(aggregate)),
            Some(standing) => {
                let reached = standing.version;

                self.carry_out(aggregate, Some(standing), reached, command, agent)
                    .await
            }
        }
    }

    pub async fn execute_at(
        &self,
        aggregate: &AggregateId,
        expected: Version,
        command: A::Command,
        agent: &Agent,
    ) -> Outcome<A> {
        match self.standing(aggregate, None).await? {
            None => Err(self.nothing_there(aggregate)),
            Some(standing) if standing.version != expected => {
                Err(ServiceError::Store(StoreError::Outdated {
                    aggregate: aggregate.clone(),
                    kind: A::KIND,
                    expected,
                }))
            }
            Some(standing) => {
                self.carry_out(aggregate, Some(standing), expected, command, agent)
                    .await
            }
        }
    }

    async fn carry_out(
        &self,
        aggregate: &AggregateId,
        standing: Option<Standing<A>>,
        expected: Version,
        command: A::Command,
        agent: &Agent,
    ) -> Outcome<A> {
        let events = match &standing {
            Some(standing) => standing.state.decide(command, agent),
            None => A::begin(command, agent),
        }
        .map_err(ServiceError::Refused)?;

        if events.is_empty() {
            return Ok(expected);
        }

        let stream = self.stamp(aggregate, expected, events, agent);
        self.store
            .append(aggregate, A::KIND, expected, &stream)
            .await?;

        let landed = stream
            .last()
            .expect("a non-empty batch was just stamped")
            .metadata
            .version;
        let state = grown(standing.map(|standing| standing.state), &stream)
            .ok_or_else(|| self.unusable(aggregate))?;

        self.snapshot_if_due(aggregate, &state, landed, agent).await
    }

    pub async fn collapse(&self, aggregate: &AggregateId, agent: &Agent) -> Outcome<A> {
        let standing = self
            .standing(aggregate, None)
            .await?
            .ok_or_else(|| self.nothing_there(aggregate))?;

        let taken = self
            .snapshot(aggregate, &standing.state, standing.version, agent)
            .await?;
        if let Some(replaced) = taken.previous() {
            self.store
                .prune_through(aggregate, A::KIND, replaced)
                .await?;
        }

        Ok(taken)
    }

    async fn standing(
        &self,
        aggregate: &AggregateId,
        through: Option<Version>,
    ) -> Result<Option<Standing<A>>, ServiceError<A::Error>> {
        let resume = match through {
            Some(version) => {
                self.store
                    .snapshot_at_or_before(aggregate, A::KIND, version)
                    .await?
            }
            None => self.store.latest_snapshot(aggregate, A::KIND).await?,
        }
        .map(|snapshot| snapshot.metadata.version)
        .unwrap_or(Version::ZERO);

        let stream = match through {
            Some(version) => {
                self.store
                    .read_through(aggregate, A::KIND, resume, version)
                    .await?
            }
            None => self.store.read_from(aggregate, A::KIND, resume).await?,
        };
        let stream: Vec<_> = stream
            .into_iter()
            .map(|entry| Recorded {
                event: self.patcher.patch(entry.event),
                metadata: entry.metadata,
            })
            .collect();

        if stream.is_empty() {
            return Ok(None);
        }

        let reached = stream
            .last()
            .expect("the stream is not empty here")
            .metadata
            .version;
        let state = grown(None, &stream).ok_or_else(|| self.unusable(aggregate))?;

        Ok(Some(Standing {
            state,
            version: reached,
        }))
    }

    fn stamp(
        &self,
        aggregate: &AggregateId,
        expected: Version,
        events: Vec<A::Event>,
        agent: &Agent,
    ) -> Vec<Recorded<A::Event>> {
        let occurred_at = self.clock.now();
        let mut version = expected;

        events
            .into_iter()
            .map(|event| {
                version = version.next();

                Recorded {
                    metadata: EventMetadata {
                        aggregate: aggregate.clone(),
                        kind: A::KIND,
                        version,
                        agent: agent.clone(),
                        occurred_at,
                        is_snapshot: event.is_snapshot(),
                    },
                    event,
                }
            })
            .collect()
    }

    async fn snapshot_if_due(
        &self,
        aggregate: &AggregateId,
        state: &A,
        landed: Version,
        agent: &Agent,
    ) -> Outcome<A> {
        let Some(every) = state.snapshot_after() else {
            return Ok(landed);
        };

        let since = self
            .store
            .latest_snapshot(aggregate, A::KIND)
            .await?
            .map(|snapshot| snapshot.metadata.version)
            .unwrap_or(Version::ZERO);

        if landed.count().saturating_sub(since.count()) < u64::from(every) {
            return Ok(landed);
        }

        self.snapshot(aggregate, state, landed, agent).await
    }

    async fn snapshot(
        &self,
        aggregate: &AggregateId,
        state: &A,
        reached: Version,
        agent: &Agent,
    ) -> Outcome<A> {
        let snapshot = state.snapshot();

        if !snapshot.is_snapshot() {
            return Err(ServiceError::Unmarked {
                aggregate: aggregate.clone(),
                kind: A::KIND,
            });
        }

        let stream = self.stamp(aggregate, reached, vec![snapshot], agent);
        self.store
            .append(aggregate, A::KIND, reached, &stream)
            .await?;

        Ok(stream
            .last()
            .expect("a snapshot is one event")
            .metadata
            .version)
    }

    fn nothing_there(&self, aggregate: &AggregateId) -> ServiceError<A::Error> {
        ServiceError::NotFound {
            aggregate: aggregate.clone(),
            kind: A::KIND,
        }
    }

    fn unusable(&self, aggregate: &AggregateId) -> ServiceError<A::Error> {
        ServiceError::Unusable {
            aggregate: aggregate.clone(),
            kind: A::KIND,
        }
    }
}

fn grown<A>(from: Option<A>, stream: &[Recorded<A::Event>]) -> Option<A>
where
    A: Aggregate,
{
    let mut state = from;

    for entry in stream {
        state = match state {
            Some(mut standing) => {
                standing.absorb(&entry.event, &entry.metadata);
                Some(standing)
            }
            None => A::born(&entry.event, &entry.metadata),
        };
    }

    state
}

#[cfg(test)]
mod tests;
