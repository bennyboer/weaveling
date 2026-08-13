use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use thiserror::Error;
use time::OffsetDateTime;
use uuid::{NoContext, Timestamp, Uuid};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectId(Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("not a valid project id")]
pub struct InvalidProjectId;

impl ProjectId {
    pub fn generate(now: OffsetDateTime) -> Self {
        Self(Uuid::new_v7(uuid_timestamp(now)))
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

fn uuid_timestamp(now: OffsetDateTime) -> Timestamp {
    let seconds = u64::try_from(now.unix_timestamp()).unwrap_or(0);

    Timestamp::from_unix(NoContext, seconds, now.nanosecond())
}

impl From<Uuid> for ProjectId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl Display for ProjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl FromStr for ProjectId {
    type Err = InvalidProjectId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self).map_err(|_| InvalidProjectId)
    }
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    #[test]
    fn ids_are_version_7() {
        let id = ProjectId::generate(at(1_000));

        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn ids_generated_at_the_same_moment_still_differ() {
        assert_ne!(
            ProjectId::generate(at(1_000)),
            ProjectId::generate(at(1_000))
        );
    }

    #[test]
    fn ids_sort_by_the_moment_they_were_generated() {
        let latest = ProjectId::generate(at(3_000));
        let earliest = ProjectId::generate(at(1_000));
        let middle = ProjectId::generate(at(2_000));

        let mut ids = vec![latest, earliest, middle];
        ids.sort();

        assert_eq!(ids, vec![earliest, middle, latest]);
    }

    #[test]
    fn ids_carry_the_moment_they_were_generated() {
        let id = ProjectId::generate(at(1_700_000_000));

        let (seconds, _) = id
            .as_uuid()
            .get_timestamp()
            .expect("a v7 id carries a timestamp")
            .to_unix();

        assert_eq!(seconds, 1_700_000_000);
    }

    #[test]
    fn display_and_parse_round_trip() {
        let id = ProjectId::generate(at(1_000));

        let parsed: ProjectId = id.to_string().parse().expect("should parse back");

        assert_eq!(id, parsed);
    }

    #[test]
    fn parsing_rejects_non_uuid() {
        assert!("weaveling".parse::<ProjectId>().is_err());
    }
}
