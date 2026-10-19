use crate::projects::model::Version;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotationRestyling {
    pub change: Version,
    pub total: u64,
    pub done: u64,
    pub finished: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restyled {
    Waiting,
    Running { done: u64, total: u64 },
    Finished,
}

pub fn standing(found: Option<QuotationRestyling>, saved_against: Version) -> Restyled {
    match found {
        Some(found) if found.change > saved_against && found.finished => Restyled::Finished,
        Some(found) if found.change > saved_against => Restyled::Running {
            done: found.done,
            total: found.total,
        },
        _ => Restyled::Waiting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(change: u64, done: u64, finished: bool) -> Option<QuotationRestyling> {
        Some(QuotationRestyling {
            change: Version::from(change),
            total: 4,
            done,
            finished,
        })
    }

    #[test]
    fn nothing_restyled_yet_is_waited_for() {
        assert_eq!(standing(None, Version::from(7)), Restyled::Waiting);
    }

    #[test]
    fn an_earlier_restyling_finished_long_ago_does_not_end_the_wait() {
        assert_eq!(
            standing(found(5, 4, true), Version::from(7)),
            Restyled::Waiting
        );
        assert_eq!(
            standing(found(7, 4, true), Version::from(7)),
            Restyled::Waiting,
            "the version saved against is from before the new style was recorded"
        );
    }

    #[test]
    fn a_restyling_newer_than_the_save_is_followed_until_it_is_finished() {
        assert_eq!(
            standing(found(8, 1, false), Version::from(7)),
            Restyled::Running { done: 1, total: 4 }
        );
        assert_eq!(
            standing(found(8, 4, true), Version::from(7)),
            Restyled::Finished
        );
    }
}
