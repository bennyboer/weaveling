use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub free: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Admitted,
    Wait(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tries {
    pub counted: u32,
    pub last: OffsetDateTime,
}

impl Tries {
    pub const FORGOTTEN_AFTER: Duration = Duration::days(1);
    const FIRST_WAIT: Duration = Duration::minutes(1);
    const LONGEST_WAIT: Duration = Duration::hours(1);

    pub fn is_forgotten(&self, now: OffsetDateTime) -> bool {
        now - self.last >= Self::FORGOTTEN_AFTER
    }

    pub fn admitted(
        before: Option<Tries>,
        rule: Rule,
        now: OffsetDateTime,
    ) -> Result<Tries, Duration> {
        let Some(before) = before.filter(|tries| !tries.is_forgotten(now)) else {
            return Ok(Tries {
                counted: 1,
                last: now,
            });
        };
        if let Some(wait) = before.wait_before_next(rule, now) {
            return Err(wait);
        }

        Ok(Tries {
            counted: before.counted.saturating_add(1),
            last: now,
        })
    }

    pub fn wait_before_next(&self, rule: Rule, now: OffsetDateTime) -> Option<Duration> {
        if self.is_forgotten(now) || self.counted < rule.free {
            return None;
        }

        let doublings = (self.counted - rule.free).min(6);
        let wait = (Self::FIRST_WAIT * 2_i32.pow(doublings)).min(Self::LONGEST_WAIT);
        let until = self.last + wait;

        (now < until).then(|| until - now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIVE_FREE: Rule = Rule { free: 5 };
    const THREE_FREE: Rule = Rule { free: 3 };

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn tried(counted: u32) -> Tries {
        Tries {
            counted,
            last: at(1_000),
        }
    }

    #[test]
    fn the_first_tries_are_free() {
        for counted in 0..5 {
            assert_eq!(tried(counted).wait_before_next(FIVE_FREE, at(1_000)), None);
        }
        for counted in 0..3 {
            assert_eq!(tried(counted).wait_before_next(THREE_FREE, at(1_000)), None);
        }
    }

    #[test]
    fn past_the_free_tries_each_wait_is_twice_the_last() {
        let waits: Vec<_> = (5..=9)
            .map(|counted| tried(counted).wait_before_next(FIVE_FREE, at(1_000)))
            .collect();

        assert_eq!(
            waits,
            [1, 2, 4, 8, 16].map(|minutes| Some(Duration::minutes(minutes))),
        );
        assert_eq!(
            tried(3).wait_before_next(THREE_FREE, at(1_000)),
            Some(Duration::minutes(1))
        );
    }

    #[test]
    fn no_wait_is_longer_than_an_hour() {
        for counted in [11, 12, 40, u32::MAX] {
            assert_eq!(
                tried(counted).wait_before_next(FIVE_FREE, at(1_000)),
                Some(Duration::hours(1)),
                "a wait without end would lock its owner out, which anyone typing their address could do"
            );
        }
    }

    #[test]
    fn a_wait_is_counted_from_the_last_try_and_shrinks_as_time_passes() {
        assert_eq!(
            tried(6).wait_before_next(FIVE_FREE, at(1_090)),
            Some(Duration::seconds(30))
        );
        assert_eq!(tried(6).wait_before_next(FIVE_FREE, at(1_120)), None);
    }

    #[test]
    fn an_admitted_try_is_counted_on_what_came_before_and_a_refused_one_is_told_its_wait() {
        assert_eq!(
            Tries::admitted(None, FIVE_FREE, at(1_000)),
            Ok(Tries {
                counted: 1,
                last: at(1_000)
            })
        );
        assert_eq!(
            Tries::admitted(Some(tried(4)), FIVE_FREE, at(1_010)),
            Ok(Tries {
                counted: 5,
                last: at(1_010)
            })
        );
        assert_eq!(
            Tries::admitted(Some(tried(5)), FIVE_FREE, at(1_010)),
            Err(Duration::seconds(50))
        );
        assert_eq!(
            Tries::admitted(
                Some(tried(40)),
                FIVE_FREE,
                at(1_000) + Tries::FORGOTTEN_AFTER
            ),
            Ok(Tries {
                counted: 1,
                last: at(1_000) + Tries::FORGOTTEN_AFTER
            }),
            "tries a day old are counted from the start again"
        );
    }

    #[test]
    fn tries_a_day_old_are_forgotten() {
        let quiet = at(1_000) + Tries::FORGOTTEN_AFTER;

        assert!(tried(40).is_forgotten(quiet));
        assert!(!tried(40).is_forgotten(quiet - Duration::seconds(1)));
        assert_eq!(tried(40).wait_before_next(FIVE_FREE, quiet), None);
    }
}
