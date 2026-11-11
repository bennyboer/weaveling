use time::{Duration, OffsetDateTime};

/// How many tries are free, how long the first wait after them is, and how long it grows at most.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub free: u32,
    pub first_wait: Duration,
    pub longest_wait: Duration,
}

impl Rule {
    /// So many tries free, then a wait doubling up to an hour.
    pub const fn free(free: u32) -> Self {
        Self {
            free,
            first_wait: Tries::FIRST_WAIT,
            longest_wait: Tries::LONGEST_WAIT,
        }
    }

    /// So many tries free, and then none until the count is forgotten: a cap rather than a wait, kept by the hour with `hour_of`.
    pub const fn capped(free: u32) -> Self {
        Self {
            free,
            first_wait: Tries::FORGOTTEN_AFTER,
            longest_wait: Tries::FORGOTTEN_AFTER,
        }
    }

    /// The same, the wait growing up to the one given instead.
    pub const fn waiting_at_most(self, longest_wait: Duration) -> Self {
        Self {
            longest_wait,
            ..self
        }
    }
}

/// What a count kept by the hour is about at this moment, the hour named beside it, so each hour is counted afresh; and how long until the next hour begins.
pub fn hour_of(about: &str, at: OffsetDateTime) -> (String, Duration) {
    let began = at
        .replace_minute(0)
        .and_then(|at| at.replace_second(0))
        .and_then(|at| at.replace_nanosecond(0))
        .expect("the start of an hour is a moment");

    (format!("{about}@{began}"), began + Duration::hours(1) - at)
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
    pub const FIRST_WAIT: Duration = Duration::minutes(1);
    pub const LONGEST_WAIT: Duration = Duration::hours(1);

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

        let doublings = (self.counted - rule.free).min(20);
        let wait = (rule.first_wait * 2_i32.pow(doublings)).min(rule.longest_wait);
        let until = self.last + wait;

        (now < until).then(|| until - now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIVE_FREE: Rule = Rule::free(5);
    const THREE_FREE: Rule = Rule::free(3);

    fn tried(counted: u32) -> Tries {
        Tries {
            counted,
            last: clock::at(1_000),
        }
    }

    #[test]
    fn past_a_cap_no_try_is_let_in_until_the_count_is_forgotten() {
        // given: a cap of three, all three tried
        let capped = Rule::capped(3);

        // expect: no wait within the cap
        assert_eq!(tried(2).wait_before_next(capped, clock::at(1_000)), None);
        // and: past it, a wait lasting until the count is forgotten, however long ago the last try was
        assert_eq!(
            tried(3).wait_before_next(capped, clock::at(1_000)),
            Some(Tries::FORGOTTEN_AFTER)
        );
        assert_eq!(
            tried(3).wait_before_next(capped, clock::at(1_000 + 3_600)),
            Some(Tries::FORGOTTEN_AFTER - Duration::hours(1)),
            "a cap does not let one more in now and then, as a doubling wait would"
        );
    }

    #[test]
    fn a_count_by_the_hour_is_about_the_hour_and_tells_the_time_left_in_it() {
        // given: two moments in one hour and one in the next
        let early = time::macros::datetime!(2026-11-11 14:00:05 UTC);
        let late = time::macros::datetime!(2026-11-11 14:59:00 UTC);
        let next = time::macros::datetime!(2026-11-11 15:00:00 UTC);

        // expect: the two in one hour are about the same, the next hour about another
        assert_eq!(hour_of("instance", early).0, hour_of("instance", late).0);
        assert_ne!(hour_of("instance", late).0, hour_of("instance", next).0);
        // and: what is left of the hour is told
        assert_eq!(hour_of("instance", late).1, Duration::minutes(1));
        assert_eq!(hour_of("instance", next).1, Duration::hours(1));
    }

    #[test]
    fn the_first_tries_are_free() {
        // expect: no wait within the free tries of either rule
        for counted in 0..5 {
            assert_eq!(
                tried(counted).wait_before_next(FIVE_FREE, clock::at(1_000)),
                None
            );
        }
        for counted in 0..3 {
            assert_eq!(
                tried(counted).wait_before_next(THREE_FREE, clock::at(1_000)),
                None
            );
        }
    }

    #[test]
    fn past_the_free_tries_each_wait_is_twice_the_last() {
        // when: the waits past five free tries are read
        let waits: Vec<_> = (5..=9)
            .map(|counted| tried(counted).wait_before_next(FIVE_FREE, clock::at(1_000)))
            .collect();

        // then: each doubles the last, starting at a minute
        assert_eq!(
            waits,
            [1, 2, 4, 8, 16].map(|minutes| Some(Duration::minutes(minutes))),
        );
        // and: another rule starts at a minute too
        assert_eq!(
            tried(3).wait_before_next(THREE_FREE, clock::at(1_000)),
            Some(Duration::minutes(1))
        );
    }

    #[test]
    fn no_wait_is_longer_than_an_hour() {
        // expect: however many tries, the wait is at most an hour
        for counted in [11, 12, 40, u32::MAX] {
            assert_eq!(
                tried(counted).wait_before_next(FIVE_FREE, clock::at(1_000)),
                Some(Duration::hours(1)),
                "a wait without end would lock its owner out, which anyone typing their address could do"
            );
        }
    }

    #[test]
    fn a_wait_is_counted_from_the_last_try_and_shrinks_as_time_passes() {
        // expect: part way through a wait, what is left of it
        assert_eq!(
            tried(6).wait_before_next(FIVE_FREE, clock::at(1_090)),
            Some(Duration::seconds(30))
        );
        // and: once it has passed, none
        assert_eq!(tried(6).wait_before_next(FIVE_FREE, clock::at(1_120)), None);
    }

    #[test]
    fn an_admitted_try_is_counted_on_what_came_before_and_a_refused_one_is_told_its_wait() {
        // expect: a first try is counted as one
        assert_eq!(
            Tries::admitted(None, FIVE_FREE, clock::at(1_000)),
            Ok(Tries {
                counted: 1,
                last: clock::at(1_000)
            })
        );
        // and: a later one is counted on the tries before it
        assert_eq!(
            Tries::admitted(Some(tried(4)), FIVE_FREE, clock::at(1_010)),
            Ok(Tries {
                counted: 5,
                last: clock::at(1_010)
            })
        );
        // and: one past the free tries is told its wait
        assert_eq!(
            Tries::admitted(Some(tried(5)), FIVE_FREE, clock::at(1_010)),
            Err(Duration::seconds(50))
        );
        // and: one a day on is counted as the first again
        assert_eq!(
            Tries::admitted(
                Some(tried(40)),
                FIVE_FREE,
                clock::at(1_000) + Tries::FORGOTTEN_AFTER
            ),
            Ok(Tries {
                counted: 1,
                last: clock::at(1_000) + Tries::FORGOTTEN_AFTER
            }),
            "tries a day old are counted from the start again"
        );
    }

    #[test]
    fn tries_a_day_old_are_forgotten() {
        // given: a day after the last try
        let quiet = clock::at(1_000) + Tries::FORGOTTEN_AFTER;

        // then: the tries are forgotten, though not a second before
        assert!(tried(40).is_forgotten(quiet));
        assert!(!tried(40).is_forgotten(quiet - Duration::seconds(1)));
        // and: there is nothing to wait for
        assert_eq!(tried(40).wait_before_next(FIVE_FREE, quiet), None);
    }

    #[test]
    fn a_rule_may_let_the_wait_grow_longer_than_an_hour() {
        // given: ten free, then waits up to a day
        let rule = Rule::free(10).waiting_at_most(Duration::days(1));

        // expect: the tenth try past the free ones waits 1024 minutes, past an hour
        assert_eq!(
            tried(20).wait_before_next(rule, clock::at(1_000)),
            Some(Duration::minutes(1_024))
        );
        // and: no wait grows past a day
        assert_eq!(
            tried(40).wait_before_next(rule, clock::at(1_000)),
            Some(Duration::days(1))
        );
        // and: the same tries under the plain rule wait an hour at most
        assert_eq!(
            tried(20).wait_before_next(Rule::free(10), clock::at(1_000)),
            Some(Duration::hours(1))
        );
    }
}
