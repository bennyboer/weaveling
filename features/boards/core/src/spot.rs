use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Spot {
    pub x: i64,
    pub y: i64,
}

impl Spot {
    pub const ORIGIN: Self = Self::at(0, 0);

    pub const fn at(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

impl Display for Spot {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spot_may_sit_anywhere_including_behind_the_origin() {
        // when: a spot is made behind the origin
        let behind = Spot::at(-4_000, -12);

        // then: it keeps where it was put
        assert_eq!(behind.x, -4_000);
        assert_eq!(behind.y, -12);
    }

    #[test]
    fn two_spots_at_the_same_place_are_the_same_spot() {
        // expect: spots are the same exactly when their places are
        assert_eq!(Spot::at(3, 4), Spot::at(3, 4));
        assert_ne!(Spot::at(3, 4), Spot::at(4, 3));
    }

    #[test]
    fn a_spot_reads_as_a_coordinate() {
        // expect: a spot reads as a coordinate
        assert_eq!(Spot::at(3, -4).to_string(), "(3, -4)");
    }
}
