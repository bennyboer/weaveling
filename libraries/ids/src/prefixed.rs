use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("not a valid id, expected a `{expected}` prefix")]
pub struct InvalidId {
    pub expected: &'static str,
}

impl InvalidId {
    pub const fn expecting(expected: &'static str) -> Self {
        Self { expected }
    }
}

#[macro_export]
macro_rules! id {
    ($name:ident, $prefix:literal) => {
        pub const PREFIX: &str = $prefix;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($crate::Uuid);

        impl $name {
            pub fn generate(now: $crate::OffsetDateTime) -> Self {
                Self($crate::sortable(now))
            }

            pub fn as_uuid(&self) -> $crate::Uuid {
                self.0
            }
        }

        impl From<$crate::Uuid> for $name {
            fn from(value: $crate::Uuid) -> Self {
                Self(value)
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}{}", PREFIX, $crate::encode(self.0))
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::InvalidId;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                text.strip_prefix(PREFIX)
                    .and_then($crate::decode)
                    .map(Self)
                    .ok_or($crate::InvalidId::expecting(PREFIX))
            }
        }

        #[cfg(test)]
        mod prefixed_id_tests {
            use super::PREFIX;
            use super::$name as Subject;

            fn at(seconds: i64) -> $crate::OffsetDateTime {
                $crate::OffsetDateTime::UNIX_EPOCH + $crate::Duration::seconds(seconds)
            }

            #[test]
            fn ids_are_version_7() {
                // expect: a generated id is a version 7 uuid
                assert_eq!(Subject::generate(at(1_000)).as_uuid().get_version_num(), 7);
            }

            #[test]
            fn ids_generated_at_the_same_moment_still_differ() {
                // expect: two ids generated at the same moment differ
                assert_ne!(Subject::generate(at(1_000)), Subject::generate(at(1_000)));
            }

            #[test]
            fn ids_sort_by_the_moment_they_were_generated() {
                // given: ids generated at three moments
                let latest = Subject::generate(at(3_000));
                let earliest = Subject::generate(at(1_000));
                let middle = Subject::generate(at(2_000));

                // when: they are sorted
                let mut ids = vec![latest, earliest, middle];
                ids.sort();

                // then: they are in the order of their moments
                assert_eq!(ids, vec![earliest, middle, latest]);
            }

            #[test]
            fn ids_carry_the_moment_they_were_generated() {
                // given: an id
                let id = Subject::generate(at(1_700_000_000));

                // when: its moment is read back
                let (seconds, _) = id
                    .as_uuid()
                    .get_timestamp()
                    .expect("a v7 id carries a timestamp")
                    .to_unix();

                // then: it is the moment it was generated at
                assert_eq!(seconds, 1_700_000_000);
            }

            #[test]
            fn display_and_parse_round_trip() {
                // given: an id
                let id = Subject::generate(at(1_000));

                // when: its text is parsed
                let parsed: Subject = id.to_string().parse().expect("should parse back");

                // then: it is the same id
                assert_eq!(id, parsed);
            }

            #[test]
            fn parsing_rejects_nonsense() {
                // expect: nonsense does not parse
                assert!("weaveling".parse::<Subject>().is_err());
            }

            #[test]
            fn the_string_form_carries_the_prefix() {
                // given: an id
                let id = Subject::generate(at(1_000));

                // then: its text starts with its prefix
                assert!(
                    id.to_string().starts_with(PREFIX),
                    "an id should say what it is: {id}"
                );
            }

            #[test]
            fn a_bare_uuid_is_not_accepted() {
                // given: a bare uuid
                let bare = Subject::generate(at(1_000)).as_uuid().to_string();

                // then: it does not parse
                assert!(
                    bare.parse::<Subject>().is_err(),
                    "an unprefixed uuid would parse as any id type"
                );
            }

            #[test]
            fn an_id_of_another_kind_is_not_accepted() {
                // given: an id of another kind
                let theirs = format!(
                    "somethingelse_{}",
                    $crate::encode(Subject::generate(at(1_000)).as_uuid())
                );

                // then: it does not parse
                assert!(
                    theirs.parse::<Subject>().is_err(),
                    "an id of another kind must not pass as this one"
                );
            }
        }
    };
}
