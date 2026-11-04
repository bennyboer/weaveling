use scenes_contract::QuotationRestylingDTO;

use crate::projects::model::Version;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotationRestyling {
    pub change: Version,
    pub total: u64,
    pub done: u64,
    pub finished: bool,
}

impl From<QuotationRestylingDTO> for QuotationRestyling {
    fn from(found: QuotationRestylingDTO) -> Self {
        Self {
            change: Version::from(found.change),
            total: found.total,
            done: found.done,
            finished: found.finished,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restyled {
    Waiting,
    Running { done: u64, total: u64 },
    Finished,
}

impl Restyled {
    pub fn advanced_to(self, found: Restyled) -> Restyled {
        if found.how_far() > self.how_far() {
            found
        } else {
            self
        }
    }

    fn how_far(self) -> (u8, u64) {
        match self {
            Restyled::Waiting => (0, 0),
            Restyled::Running { done, .. } => (1, done),
            Restyled::Finished => (2, 0),
        }
    }
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
        // expect: no restyling found is waited for
        assert_eq!(standing(None, Version::from(7)), Restyled::Waiting);
    }

    #[test]
    fn an_earlier_restyling_finished_long_ago_does_not_end_the_wait() {
        // expect: a finished restyling not newer than the save is waited past
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
        // expect: a newer restyling still going shows how far it is
        assert_eq!(
            standing(found(8, 1, false), Version::from(7)),
            Restyled::Running { done: 1, total: 4 }
        );
        // and: once finished it is finished
        assert_eq!(
            standing(found(8, 4, true), Version::from(7)),
            Restyled::Finished
        );
    }

    #[test]
    fn a_restyling_followed_goes_only_forward() {
        // given: a restyling halfway through
        let halfway = Restyled::Running { done: 2, total: 4 };

        // when: it is found further along
        // then: it moves on
        assert_eq!(
            halfway.advanced_to(Restyled::Running { done: 3, total: 4 }),
            Restyled::Running { done: 3, total: 4 }
        );
        assert_eq!(halfway.advanced_to(Restyled::Finished), Restyled::Finished);

        // when: it is found less far along, by an answer read earlier
        // then: it stays where it is
        assert_eq!(
            halfway.advanced_to(Restyled::Running { done: 1, total: 4 }),
            halfway
        );
        assert_eq!(
            halfway.advanced_to(Restyled::Waiting),
            halfway,
            "an answer read before the restyling began arrives after its first progress"
        );
        assert_eq!(
            Restyled::Finished.advanced_to(Restyled::Waiting),
            Restyled::Finished,
            "a late answer must not take a finished restyling back to waiting"
        );
    }
}
