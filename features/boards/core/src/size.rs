use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

impl Size {
    pub const CARD: Self = Self::of(168, 84);

    pub const fn of(width: i64, height: i64) -> Self {
        Self { width, height }
    }

    pub fn has_extent(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}\u{00d7}{}", self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_keeps_the_extent_it_was_given() {
        let wide = Size::of(400, 90);

        assert_eq!(wide.width, 400);
        assert_eq!(wide.height, 90);
    }

    #[test]
    fn a_card_with_no_extent_in_either_direction_is_not_a_card() {
        assert!(Size::of(1, 1).has_extent());
        assert!(!Size::of(0, 84).has_extent());
        assert!(!Size::of(168, 0).has_extent());
        assert!(!Size::of(-168, 84).has_extent());
        assert!(!Size::of(168, -84).has_extent());
    }

    #[test]
    fn a_size_reads_as_an_extent() {
        assert_eq!(Size::of(168, 84).to_string(), "168\u{00d7}84");
    }
}
