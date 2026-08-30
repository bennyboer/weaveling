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

    fn key(of: &str) -> RoutingKey {
        RoutingKey::parse(of).expect("a plain key is fine")
    }

    fn listening(to: &str) -> Subscription {
        Subscription::parse(to).expect("a plain pattern is fine")
    }

    #[test]
    fn a_key_reads_back_as_it_was_written() {
        assert_eq!(key("piece.captured").to_string(), "piece.captured");
        assert_eq!(key("piece.captured").parts(), ["piece", "captured"]);
    }

    #[test]
    fn an_empty_key_is_refused() {
        assert_eq!(RoutingKey::parse(""), Err(InvalidRoutingKey::Empty));
    }

    #[test]
    fn a_key_with_a_hole_in_it_is_refused() {
        assert_eq!(
            RoutingKey::parse("piece..captured"),
            Err(InvalidRoutingKey::EmptyPart)
        );
        assert_eq!(
            RoutingKey::parse("piece."),
            Err(InvalidRoutingKey::EmptyPart)
        );
    }

    #[test]
    fn a_published_key_may_not_carry_wildcards() {
        assert_eq!(
            RoutingKey::parse("piece.*"),
            Err(InvalidRoutingKey::Wildcard)
        );
        assert_eq!(
            RoutingKey::parse("piece.#"),
            Err(InvalidRoutingKey::Wildcard)
        );
    }

    #[test]
    fn a_subscription_may_carry_wildcards() {
        assert!(Subscription::parse("piece.*").is_ok());
        assert!(Subscription::parse("#").is_ok());
    }

    #[test]
    fn an_exact_subscription_covers_only_that_key() {
        let listening = listening("piece.captured");

        assert!(listening.covers(&key("piece.captured")));
        assert!(!listening.covers(&key("piece.retitled")));
        assert!(!listening.covers(&key("piece.captured.late")));
        assert!(!listening.covers(&key("piece")));
    }

    #[test]
    fn a_single_wildcard_stands_for_exactly_one_part() {
        let listening = listening("piece.*");

        assert!(listening.covers(&key("piece.captured")));
        assert!(listening.covers(&key("piece.retitled")));
        assert!(
            !listening.covers(&key("piece.captured.late")),
            "one wildcard must not swallow two parts"
        );
        assert!(!listening.covers(&key("board.captured")));
    }

    #[test]
    fn a_wildcard_may_sit_anywhere() {
        let listening = listening("*.captured");

        assert!(listening.covers(&key("piece.captured")));
        assert!(listening.covers(&key("board.captured")));
        assert!(!listening.covers(&key("piece.retitled")));
    }

    #[test]
    fn the_open_wildcard_stands_for_any_number_of_parts() {
        let listening = listening("piece.#");

        assert!(listening.covers(&key("piece.captured")));
        assert!(listening.covers(&key("piece.captured.late")));
        assert!(
            listening.covers(&key("piece")),
            "the open wildcard also stands for nothing at all"
        );
        assert!(!listening.covers(&key("board.captured")));
    }

    #[test]
    fn everything_is_covered_by_the_open_wildcard_alone() {
        let listening = listening("#");

        assert!(listening.covers(&key("piece.captured")));
        assert!(listening.covers(&key("board")));
        assert!(listening.covers(&key("a.very.deeply.nested.key")));
    }

    #[test]
    fn the_open_wildcard_works_in_the_middle_too() {
        let listening = listening("piece.#.late");

        assert!(listening.covers(&key("piece.captured.late")));
        assert!(listening.covers(&key("piece.captured.very.late")));
        assert!(listening.covers(&key("piece.late")));
        assert!(!listening.covers(&key("piece.captured")));
    }

    #[test]
    fn a_subscription_reads_back_as_it_was_written() {
        assert_eq!(listening("piece.#").to_string(), "piece.#");
    }
}
