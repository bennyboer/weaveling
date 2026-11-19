use clock::text;
use eventsourcing::{AccountId, Agent, Codec};
use readings_core::{Extent, ProjectLink, ReadingCopyEvent, ReadingCopyName, Times};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A reading copy's event as the event store keeps it.
/// Its form is the store's own, apart from the aggregate's and the contract's, so it changes only together with what reads it back.
#[derive(Serialize, Deserialize)]
enum StoredReadingCopyEvent {
    Made {
        project: String,
        name: String,
        extent: StoredExtent,
        times: StoredTimes,
    },
    Renamed {
        name: String,
    },
    Extended {
        times: StoredTimes,
    },
    Deleted,
    Snapshotted {
        project: String,
        name: String,
        extent: StoredExtent,
        times: StoredTimes,
        made_by: StoredAgent,
        made_at: String,
        deleted: bool,
    },
}

#[derive(Serialize, Deserialize)]
enum StoredExtent {
    Book,
    Sections(u32),
    Scenes(u32),
}

#[derive(Serialize, Deserialize)]
struct StoredTimes {
    open_until: Option<String>,
    deleted_on: Option<String>,
}

#[derive(Serialize, Deserialize)]
enum StoredAgent {
    System,
    Local,
    Account(String),
}

pub fn codec() -> Codec<ReadingCopyEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredReadingCopyEvent::from(event))
                .expect("a stored reading copy event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredReadingCopyEvent>(body)
                .ok()
                .and_then(|stored| ReadingCopyEvent::try_from(stored).ok())
        },
    }
}

/// Something the store holds that no longer reads back as an event.
#[derive(Debug)]
struct Unreadable;

fn named(name: &str) -> Result<ReadingCopyName, Unreadable> {
    ReadingCopyName::new(name).map_err(|_| Unreadable)
}

fn moment(written: &str) -> Result<OffsetDateTime, Unreadable> {
    text::read(written).ok_or(Unreadable)
}

fn maybe_moment(written: Option<&str>) -> Result<Option<OffsetDateTime>, Unreadable> {
    written.map(moment).transpose()
}

impl From<Extent> for StoredExtent {
    fn from(extent: Extent) -> Self {
        match extent {
            Extent::Book => Self::Book,
            Extent::Sections(count) => Self::Sections(count),
            Extent::Scenes(count) => Self::Scenes(count),
        }
    }
}

impl From<StoredExtent> for Extent {
    fn from(stored: StoredExtent) -> Self {
        match stored {
            StoredExtent::Book => Self::Book,
            StoredExtent::Sections(count) => Self::Sections(count),
            StoredExtent::Scenes(count) => Self::Scenes(count),
        }
    }
}

impl From<Times> for StoredTimes {
    fn from(times: Times) -> Self {
        Self {
            open_until: times.open_until.map(text::written),
            deleted_on: times.deleted_on.map(text::written),
        }
    }
}

impl TryFrom<StoredTimes> for Times {
    type Error = Unreadable;

    fn try_from(stored: StoredTimes) -> Result<Self, Unreadable> {
        Ok(Self {
            open_until: maybe_moment(stored.open_until.as_deref())?,
            deleted_on: maybe_moment(stored.deleted_on.as_deref())?,
        })
    }
}

impl From<&Agent> for StoredAgent {
    fn from(agent: &Agent) -> Self {
        match agent {
            Agent::System => Self::System,
            Agent::Local => Self::Local,
            Agent::Account(account) => Self::Account(account.to_string()),
        }
    }
}

impl From<StoredAgent> for Agent {
    fn from(stored: StoredAgent) -> Self {
        match stored {
            StoredAgent::System => Self::System,
            StoredAgent::Local => Self::Local,
            StoredAgent::Account(account) => Self::Account(AccountId::from(account)),
        }
    }
}

impl From<&ReadingCopyEvent> for StoredReadingCopyEvent {
    fn from(event: &ReadingCopyEvent) -> Self {
        match event {
            ReadingCopyEvent::Made {
                project,
                name,
                extent,
                times,
            } => Self::Made {
                project: project.to_string(),
                name: name.to_string(),
                extent: StoredExtent::from(*extent),
                times: StoredTimes::from(*times),
            },
            ReadingCopyEvent::Renamed(name) => Self::Renamed {
                name: name.to_string(),
            },
            ReadingCopyEvent::Extended(times) => Self::Extended {
                times: StoredTimes::from(*times),
            },
            ReadingCopyEvent::Deleted => Self::Deleted,
            ReadingCopyEvent::Snapshotted {
                project,
                name,
                extent,
                times,
                made_by,
                made_at,
                deleted,
            } => Self::Snapshotted {
                project: project.to_string(),
                name: name.to_string(),
                extent: StoredExtent::from(*extent),
                times: StoredTimes::from(*times),
                made_by: StoredAgent::from(made_by),
                made_at: text::written(*made_at),
                deleted: *deleted,
            },
        }
    }
}

impl TryFrom<StoredReadingCopyEvent> for ReadingCopyEvent {
    type Error = Unreadable;

    fn try_from(stored: StoredReadingCopyEvent) -> Result<Self, Unreadable> {
        Ok(match stored {
            StoredReadingCopyEvent::Made {
                project,
                name,
                extent,
                times,
            } => Self::Made {
                project: ProjectLink::from(project),
                name: named(&name)?,
                extent: Extent::from(extent),
                times: Times::try_from(times)?,
            },
            StoredReadingCopyEvent::Renamed { name } => Self::Renamed(named(&name)?),
            StoredReadingCopyEvent::Extended { times } => Self::Extended(Times::try_from(times)?),
            StoredReadingCopyEvent::Deleted => Self::Deleted,
            StoredReadingCopyEvent::Snapshotted {
                project,
                name,
                extent,
                times,
                made_by,
                made_at,
                deleted,
            } => Self::Snapshotted {
                project: ProjectLink::from(project),
                name: named(&name)?,
                extent: Extent::from(extent),
                times: Times::try_from(times)?,
                made_by: Agent::from(made_by),
                made_at: moment(&made_at)?,
                deleted,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    const AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    fn a_name(saying: &str) -> ReadingCopyName {
        ReadingCopyName::new(saying).expect("a plain name is fine")
    }

    fn times() -> Times {
        Times {
            open_until: Some(AT + Duration::days(30)),
            deleted_on: None,
        }
    }

    fn kept_and_read_back(event: &ReadingCopyEvent) -> Option<ReadingCopyEvent> {
        let codec = codec();

        (codec.event)((codec.body)(event))
    }

    #[test]
    fn every_event_reads_back_as_it_was_kept() {
        // given: every kind of event, a snapshot by each kind of agent among them
        let mut every = vec![
            ReadingCopyEvent::Made {
                project: ProjectLink::from("project_loom"),
                name: a_name("For Lena"),
                extent: Extent::Sections(3),
                times: times(),
            },
            ReadingCopyEvent::Renamed(a_name("For Lena, again")),
            ReadingCopyEvent::Extended(times()),
            ReadingCopyEvent::Deleted,
        ];
        for made_by in [
            Agent::System,
            Agent::Local,
            Agent::Account(AccountId::from("account_hedy")),
        ] {
            every.push(ReadingCopyEvent::Snapshotted {
                project: ProjectLink::from("project_loom"),
                name: a_name("For Lena"),
                extent: Extent::Scenes(2),
                times: times(),
                made_by,
                made_at: AT + Duration::seconds(7),
                deleted: true,
            });
        }

        // expect: each reads back as it was kept
        for event in every {
            assert_eq!(
                kept_and_read_back(&event).as_ref(),
                Some(&event),
                "{event:?}"
            );
        }
    }

    #[test]
    fn what_no_longer_reads_back_is_no_event() {
        // given: a kept rename whose name could no longer be a name
        let kept = serde_json::json!({ "Renamed": { "name": "  " } });

        // expect: it reads back as nothing, rather than as a copy with no name
        assert_eq!((codec().event)(kept), None);
    }
}
