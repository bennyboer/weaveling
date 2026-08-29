use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use thiserror::Error;
use time::OffsetDateTime;
use uuid::{NoContext, Timestamp, Uuid};

pub const PREFIX: &str = "project_";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectId(Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("not a valid project id, expected a `{PREFIX}` prefix")]
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
        write!(f, "{PREFIX}{}", ids::encode(self.0))
    }
}

impl FromStr for ProjectId {
    type Err = InvalidProjectId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix(PREFIX)
            .and_then(ids::decode)
            .map(Self)
            .ok_or(InvalidProjectId)
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

    #[test]
    fn the_string_form_carries_the_prefix() {
        let id = ProjectId::generate(at(1_000));

        assert!(
            id.to_string().starts_with("project_"),
            "an id should say what it is: {id}"
        );
    }

    #[test]
    fn a_bare_uuid_is_not_accepted() {
        let bare = ProjectId::generate(at(1_000)).as_uuid().to_string();

        assert!(
            bare.parse::<ProjectId>().is_err(),
            "an unprefixed uuid would parse as any id type"
        );
    }

    #[test]
    fn an_id_of_another_kind_is_not_accepted() {
        let theirs = format!(
            "passage_{}",
            ids::encode(ProjectId::generate(at(1_000)).as_uuid())
        );

        assert!(
            theirs.parse::<ProjectId>().is_err(),
            "an id of another kind must not pass as this one"
        );
    }
}
