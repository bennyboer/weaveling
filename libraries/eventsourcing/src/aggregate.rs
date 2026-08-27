use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AggregateId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AggregateType(&'static str);

impl AggregateId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AggregateId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for AggregateId {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for AggregateId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl AggregateType {
    pub const fn of(named: &'static str) -> Self {
        Self(named)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl Display for AggregateType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIECE: AggregateType = AggregateType::of("piece");

    #[test]
    fn an_aggregate_type_is_nameable_at_compile_time() {
        assert_eq!(PIECE.as_str(), "piece");
        assert_eq!(PIECE.to_string(), "piece");
    }

    #[test]
    fn two_kinds_of_aggregate_are_not_the_same() {
        assert_ne!(PIECE, AggregateType::of("board"));
    }

    #[test]
    fn an_aggregate_id_keeps_whatever_the_feature_gave_it() {
        let id = AggregateId::from("piece_019a4f2b");

        assert_eq!(id.as_str(), "piece_019a4f2b");
        assert_eq!(id.to_string(), "piece_019a4f2b");
    }
}
