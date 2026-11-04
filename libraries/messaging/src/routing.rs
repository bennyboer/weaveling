use std::fmt::{self, Display, Formatter};

use thiserror::Error;

const SEPARATOR: char = '.';
const ONE: &str = "*";
const REST: &str = "#";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoutingKey(Vec<String>);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidRoutingKey {
    #[error("a routing key must not be empty")]
    Empty,
    #[error("a routing key must not have an empty part")]
    EmptyPart,
    #[error("a routing key must not contain wildcards, only a subscription may")]
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subscription(Vec<String>);

impl RoutingKey {
    pub fn parse(key: &str) -> Result<Self, InvalidRoutingKey> {
        let parts = split(key)?;

        if parts.iter().any(|part| part == ONE || part == REST) {
            return Err(InvalidRoutingKey::Wildcard);
        }

        Ok(Self(parts))
    }

    pub fn parts(&self) -> &[String] {
        &self.0
    }
}

impl Subscription {
    pub fn parse(pattern: &str) -> Result<Self, InvalidRoutingKey> {
        Ok(Self(split(pattern)?))
    }

    pub fn covers(&self, key: &RoutingKey) -> bool {
        covers(&self.0, key.parts())
    }
}

fn split(key: &str) -> Result<Vec<String>, InvalidRoutingKey> {
    if key.is_empty() {
        return Err(InvalidRoutingKey::Empty);
    }

    let parts: Vec<String> = key.split(SEPARATOR).map(str::to_owned).collect();

    if parts.iter().any(String::is_empty) {
        return Err(InvalidRoutingKey::EmptyPart);
    }

    Ok(parts)
}

fn covers(pattern: &[String], key: &[String]) -> bool {
    match (pattern.first(), key.first()) {
        (None, None) => true,
        (Some(head), _) if head == REST => {
            covers(&pattern[1..], key) || (!key.is_empty() && covers(pattern, &key[1..]))
        }
        (Some(head), Some(part)) if head == ONE || head == part => covers(&pattern[1..], &key[1..]),
        _ => false,
    }
}

impl Display for RoutingKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("."))
    }
}

impl Display for Subscription {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_key(of: &str) -> RoutingKey {
        RoutingKey::parse(of).expect("a plain key is fine")
    }

    fn a_subscription(to: &str) -> Subscription {
        Subscription::parse(to).expect("a plain pattern is fine")
    }

    #[test]
    fn a_key_reads_back_as_it_was_written() {
        // expect: a key reads back as it was written, part by part
        assert_eq!(a_key("idea.captured").to_string(), "idea.captured");
        assert_eq!(a_key("idea.captured").parts(), ["idea", "captured"]);
    }

    #[test]
    fn an_empty_key_is_refused() {
        // expect: an empty key is refused
        assert_eq!(RoutingKey::parse(""), Err(InvalidRoutingKey::Empty));
    }

    #[test]
    fn a_key_with_a_hole_in_it_is_refused() {
        // expect: a key with an empty part is refused
        assert_eq!(
            RoutingKey::parse("idea..captured"),
            Err(InvalidRoutingKey::EmptyPart)
        );
        assert_eq!(
            RoutingKey::parse("idea."),
            Err(InvalidRoutingKey::EmptyPart)
        );
    }

    #[test]
    fn a_published_key_may_not_carry_wildcards() {
        // expect: a key with a wildcard is refused
        assert_eq!(
            RoutingKey::parse("idea.*"),
            Err(InvalidRoutingKey::Wildcard)
        );
        assert_eq!(
            RoutingKey::parse("idea.#"),
            Err(InvalidRoutingKey::Wildcard)
        );
    }

    #[test]
    fn a_subscription_may_carry_wildcards() {
        // expect: a subscription with wildcards is taken
        assert!(Subscription::parse("idea.*").is_ok());
        assert!(Subscription::parse("#").is_ok());
    }

    #[test]
    fn an_exact_subscription_covers_only_that_key() {
        // given: a subscription to one key
        let listening = a_subscription("idea.captured");

        // then: it covers that key alone
        assert!(listening.covers(&a_key("idea.captured")));
        assert!(!listening.covers(&a_key("idea.retitled")));
        assert!(!listening.covers(&a_key("idea.captured.late")));
        assert!(!listening.covers(&a_key("idea")));
    }

    #[test]
    fn a_single_wildcard_stands_for_exactly_one_part() {
        // given: a subscription ending in a single wildcard
        let listening = a_subscription("idea.*");

        // then: it covers keys with exactly one part there
        assert!(listening.covers(&a_key("idea.captured")));
        assert!(listening.covers(&a_key("idea.retitled")));
        assert!(
            !listening.covers(&a_key("idea.captured.late")),
            "one wildcard must not swallow two parts"
        );
        assert!(!listening.covers(&a_key("board.captured")));
    }

    #[test]
    fn a_wildcard_may_sit_anywhere() {
        // given: a subscription starting with a single wildcard
        let listening = a_subscription("*.captured");

        // then: it covers keys with any one part there
        assert!(listening.covers(&a_key("idea.captured")));
        assert!(listening.covers(&a_key("board.captured")));
        assert!(!listening.covers(&a_key("idea.retitled")));
    }

    #[test]
    fn the_open_wildcard_stands_for_any_number_of_parts() {
        // given: a subscription ending in the open wildcard
        let listening = a_subscription("idea.#");

        // then: it covers keys with any number of parts there, none too
        assert!(listening.covers(&a_key("idea.captured")));
        assert!(listening.covers(&a_key("idea.captured.late")));
        assert!(
            listening.covers(&a_key("idea")),
            "the open wildcard also stands for nothing at all"
        );
        assert!(!listening.covers(&a_key("board.captured")));
    }

    #[test]
    fn everything_is_covered_by_the_open_wildcard_alone() {
        // given: a subscription of the open wildcard alone
        let listening = a_subscription("#");

        // then: it covers every key
        assert!(listening.covers(&a_key("idea.captured")));
        assert!(listening.covers(&a_key("board")));
        assert!(listening.covers(&a_key("a.very.deeply.nested.key")));
    }

    #[test]
    fn the_open_wildcard_works_in_the_middle_too() {
        // given: a subscription with the open wildcard in its middle
        let listening = a_subscription("idea.#.late");

        // then: it covers keys with any number of parts there, none too
        assert!(listening.covers(&a_key("idea.captured.late")));
        assert!(listening.covers(&a_key("idea.captured.very.late")));
        assert!(listening.covers(&a_key("idea.late")));
        assert!(!listening.covers(&a_key("idea.captured")));
    }

    #[test]
    fn a_subscription_reads_back_as_it_was_written() {
        // expect: a subscription reads back as it was written
        assert_eq!(a_subscription("idea.#").to_string(), "idea.#");
    }
}
