use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use thiserror::Error;
use time::OffsetDateTime;
use uuid::{NoContext, Timestamp, Uuid};

pub const PREFIX: &str = "passage_";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PassageId(Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("not a valid passage id, expected a `{PREFIX}` prefix")]
pub struct InvalidPassageId;

impl PassageId {
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

impl From<Uuid> for PassageId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl Display for PassageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{PREFIX}{}", self.0)
    }
}

impl FromStr for PassageId {
    type Err = InvalidPassageId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix(PREFIX)
            .and_then(|raw| Uuid::parse_str(raw).ok())
            .map(Self)
            .ok_or(InvalidPassageId)
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
        let id = PassageId::generate(at(1_000));

        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn ids_generated_at_the_same_moment_still_differ() {
        assert_ne!(
            PassageId::generate(at(1_000)),
            PassageId::generate(at(1_000))
        );
    }

    #[test]
    fn ids_sort_by_the_moment_they_were_generated() {
        let latest = PassageId::generate(at(3_000));
        let earliest = PassageId::generate(at(1_000));
        let middle = PassageId::generate(at(2_000));

        let mut ids = vec![latest, earliest, middle];
        ids.sort();

        assert_eq!(ids, vec![earliest, middle, latest]);
    }

    #[test]
    fn display_and_parse_round_trip() {
        let id = PassageId::generate(at(1_000));

        let parsed: PassageId = id.to_string().parse().expect("should parse back");

        assert_eq!(id, parsed);
    }

    #[test]
    fn parsing_rejects_non_uuid() {
        assert!("weaveling".parse::<PassageId>().is_err());
    }

    #[test]
    fn the_string_form_carries_the_prefix() {
        let id = PassageId::generate(at(1_000));

        assert!(
            id.to_string().starts_with("passage_"),
            "an id should say what it is: {id}"
        );
    }

    #[test]
    fn a_bare_uuid_is_not_accepted() {
        let bare = PassageId::generate(at(1_000)).as_uuid().to_string();

        assert!(
            bare.parse::<PassageId>().is_err(),
            "an unprefixed uuid would parse as any id type"
        );
    }

    #[test]
    fn an_id_of_another_kind_is_not_accepted() {
        let theirs = format!("project_{}", PassageId::generate(at(1_000)).as_uuid());

        assert!(
            theirs.parse::<PassageId>().is_err(),
            "an id of another kind must not pass as this one"
        );
    }
}
