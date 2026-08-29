use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(u64);

impl Version {
    pub const ZERO: Self = Self(0);

    pub const fn of(counted: u64) -> Self {
        Self(counted)
    }

    pub fn count(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn previous(self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }

    pub fn precedes(self, other: Self) -> bool {
        self.0.checked_add(1) == Some(other.0)
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_version_is_zero() {
        assert_eq!(Version::ZERO.count(), 0);
    }

    #[test]
    fn the_next_version_counts_one_higher() {
        assert_eq!(Version::ZERO.next(), Version::of(1));
        assert_eq!(Version::of(41).next(), Version::of(42));
    }

    #[test]
    fn nothing_precedes_the_first_version() {
        assert_eq!(Version::ZERO.previous(), None);
        assert_eq!(Version::of(1).previous(), Some(Version::ZERO));
    }

    #[test]
    fn a_version_precedes_only_the_one_directly_after_it() {
        assert!(Version::of(7).precedes(Version::of(8)));
        assert!(!Version::of(7).precedes(Version::of(9)));
        assert!(!Version::of(7).precedes(Version::of(7)));
        assert!(!Version::of(7).precedes(Version::of(6)));
    }

    #[test]
    fn the_last_countable_version_precedes_nothing() {
        assert!(!Version::of(u64::MAX).precedes(Version::ZERO));
    }

    #[test]
    fn versions_order_by_count() {
        let mut seen = vec![Version::of(3), Version::ZERO, Version::of(1)];
        seen.sort();

        assert_eq!(seen, vec![Version::ZERO, Version::of(1), Version::of(3)]);
    }

    #[test]
    fn a_version_reads_as_its_count() {
        assert_eq!(Version::of(12).to_string(), "12");
    }
}
