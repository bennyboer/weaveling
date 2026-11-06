use std::sync::Arc;

use clock::Clock;
use thiserror::Error;
use transacting::{Failure, LostRace, Transaction};

use crate::agent::Agent;
use crate::aggregate::{Aggregate, AggregateId, AggregateType};
use crate::context::AggregateContext;
use crate::event::{Event, Recorded};
use crate::metadata::EventMetadata;
use crate::patch::Patcher;
use crate::store::{EventStore, StoreError};
use crate::version::Version;

#[derive(Debug, Error)]
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
        "the snapshot for {kind} {aggregate} does not declare itself a snapshot, so it could never \
         be found again"
    )]
    Unmarked {
        aggregate: AggregateId,
        kind: AggregateType,
    },
    #[error(transparent)]
    Refused(E),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the transaction around the command failed")]
    Transaction(#[from] Failure),
}

impl<E> LostRace for ServiceError<E> {
    fn lost_race(&self) -> bool {
        matches!(self, ServiceError::Store(StoreError::Outdated { .. }))
    }
}

pub struct EventSourcingService<A: Aggregate> {
    store: Arc<dyn EventStore<A::Event>>,
    patcher: Patcher<A::Event>,
    clock: Arc<dyn Clock>,
}

pub fn tries_for(expected: Option<Version>) -> u8 {
    match expected {
        Some(_) => 1,
        None => transacting::retrying::TRIES,
    }
}

type Outcome<A> = Result<Appended<<A as Aggregate>::Event>, ServiceError<<A as Aggregate>::Error>>;
type Reached<A> = Result<Version, ServiceError<<A as Aggregate>::Error>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing<A> {
    pub state: A,
    pub version: Version,
}

struct Decided<A: Aggregate> {
    stream: Vec<Recorded<A::Event>>,
    state: A,
    landed: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appended<E> {
    pub version: Version,
    pub events: Vec<Recorded<E>>,
}

impl<A> EventSourcingService<A>
where
    A: Aggregate,
    A::Event: Clone + Send + Sync + 'static,
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

    pub async fn latest_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
    ) -> Result<Standing<A>, ServiceError<A::Error>> {
        self.standing_within(transaction, aggregate)
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
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        command: A::Command,
        agent: &Agent,
    ) -> Outcome<A> {
        self.appended_within(transaction, aggregate, None, Version::ZERO, command, agent)
            .await
    }

    pub async fn execute(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        command: A::Command,
        agent: &Agent,
    ) -> Outcome<A> {
        self.execute_at(transaction, aggregate, None, command, agent)
            .await
    }

    pub async fn execute_at(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        expected: Option<Version>,
        command: A::Command,
        agent: &Agent,
    ) -> Outcome<A> {
        match self.standing_within(transaction, aggregate).await? {
            None => Err(self.nothing_there(aggregate)),
            Some(standing) => match expected {
                Some(expected) if standing.version != expected => Err(ServiceError::Store(
                    StoreError::outdated(aggregate, A::KIND, expected),
                )),
                _ => {
                    let reached = standing.version;
                    self.appended_within(
                        transaction,
                        aggregate,
                        Some(standing),
                        reached,
                        command,
                        agent,
                    )
                    .await
                }
            },
        }
    }

    async fn appended_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        standing: Option<Standing<A>>,
        expected: Version,
        command: A::Command,
        agent: &Agent,
    ) -> Outcome<A> {
        let Some(decided) = self.decided(aggregate, standing, expected, command, agent)? else {
            return Ok(Appended::nothing_at(expected));
        };
        self.store
            .append(transaction, aggregate, A::KIND, expected, &decided.stream)
            .await?;
        let version = self
            .snapshot_if_due(
                transaction,
                aggregate,
                &decided.state,
                decided.landed,
                agent,
            )
            .await?;

        Ok(Appended {
            version,
            events: decided.stream,
        })
    }

    fn decided(
        &self,
        aggregate: &AggregateId,
        standing: Option<Standing<A>>,
        expected: Version,
        command: A::Command,
        agent: &Agent,
    ) -> Result<Option<Decided<A>>, ServiceError<A::Error>> {
        let events = match &standing {
            Some(standing) => standing.state.decide(command, agent),
            None => A::begin(command, agent),
        }
        .map_err(ServiceError::Refused)?;

        if events.is_empty() {
            return Ok(None);
        }

        let mut stream = self.stamp(
            aggregate,
            expected,
            events,
            agent,
            &AggregateContext::default(),
        );
        let state = grown(standing.map(|standing| standing.state), &stream)
            .ok_or_else(|| self.unusable(aggregate))?;
        let context = state.context();
        for happened in &mut stream {
            happened.metadata.context = context.clone();
        }
        let landed = stream
            .last()
            .expect("a non-empty batch was just stamped")
            .metadata
            .version;

        Ok(Some(Decided {
            stream,
            state,
            landed,
        }))
    }

    pub async fn compact(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        agent: &Agent,
    ) -> Reached<A> {
        let standing = self
            .standing_within(transaction, aggregate)
            .await?
            .ok_or_else(|| self.nothing_there(aggregate))?;

        let taken = self
            .snapshot(
                transaction,
                aggregate,
                &standing.state,
                standing.version,
                agent,
            )
            .await?;
        if let Some(replaced) = taken.previous() {
            self.store
                .prune_through(transaction, aggregate, A::KIND, replaced)
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
        self.grown_from(aggregate, stream)
    }

    async fn standing_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
    ) -> Result<Option<Standing<A>>, ServiceError<A::Error>> {
        let resume = self
            .store
            .latest_snapshot_within(transaction, aggregate, A::KIND)
            .await?
            .map(|snapshot| snapshot.metadata.version)
            .unwrap_or(Version::ZERO);
        let stream = self
            .store
            .read_from_within(transaction, aggregate, A::KIND, resume)
            .await?;

        self.grown_from(aggregate, stream)
    }

    fn grown_from(
        &self,
        aggregate: &AggregateId,
        stream: Vec<Recorded<A::Event>>,
    ) -> Result<Option<Standing<A>>, ServiceError<A::Error>> {
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
        context: &AggregateContext,
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
                        context: context.clone(),
                    },
                    event,
                }
            })
            .collect()
    }

    async fn snapshot_if_due(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        state: &A,
        landed: Version,
        agent: &Agent,
    ) -> Reached<A> {
        let Some(every) = state.snapshot_after() else {
            return Ok(landed);
        };

        let since = self
            .store
            .latest_snapshot_within(transaction, aggregate, A::KIND)
            .await?
            .map(|snapshot| snapshot.metadata.version)
            .unwrap_or(Version::ZERO);

        if landed.count().saturating_sub(since.count()) < u64::from(every) {
            return Ok(landed);
        }

        self.snapshot(transaction, aggregate, state, landed, agent)
            .await
    }

    async fn snapshot(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        state: &A,
        reached: Version,
        agent: &Agent,
    ) -> Reached<A> {
        let snapshot = state.snapshot();

        if !snapshot.is_snapshot() {
            return Err(ServiceError::Unmarked {
                aggregate: aggregate.clone(),
                kind: A::KIND,
            });
        }

        let stream = self.stamp(aggregate, reached, vec![snapshot], agent, &state.context());
        self.store
            .append(transaction, aggregate, A::KIND, reached, &stream)
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

impl<E> Appended<E> {
    fn nothing_at(version: Version) -> Self {
        Self {
            version,
            events: Vec::new(),
        }
    }

    pub fn changed_nothing(&self) -> bool {
        self.events.is_empty()
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
                standing.apply(&entry.event, &entry.metadata);
                Some(standing)
            }
            None => A::from_first(&entry.event, &entry.metadata),
        };
    }

    state
}

#[cfg(test)]
mod tests;
