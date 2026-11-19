use time::{Date, Duration, OffsetDateTime, Time, UtcOffset};

/// How long a copy is open for reading and feedback, as chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenFor {
    Days(i64),
    NoEnd,
    Until(Date),
}

/// How long a copy is kept before it is deleted with its feedback, as chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptFor {
    Days(i64),
    Ever,
    Until(Date),
}

/// The moments a copy closes and is deleted, from what was chosen.
/// A copy cannot stay open beyond its deletion, so it closes when it is deleted if that comes first.
pub fn times(
    now: OffsetDateTime,
    here: UtcOffset,
    open: OpenFor,
    kept: KeptFor,
) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    let deleted_on = match kept {
        KeptFor::Days(days) => Some(now + Duration::days(days)),
        KeptFor::Ever => None,
        KeptFor::Until(day) => Some(end_of(day, here)),
    };
    let open_until = match open {
        OpenFor::Days(days) => Some(now + Duration::days(days)),
        OpenFor::NoEnd => None,
        OpenFor::Until(day) => Some(end_of(day, here)),
    };
    let open_until = match (open_until, deleted_on) {
        (Some(open), Some(deleted)) => Some(open.min(deleted)),
        (None, deleted) => deleted,
        (open, None) => open,
    };

    (open_until, deleted_on)
}

/// The last moment of a day where the reader is.
fn end_of(day: Date, here: UtcOffset) -> OffsetDateTime {
    day.with_time(Time::from_hms(23, 59, 59).expect("a time of day"))
        .assume_offset(here)
}

/// The run of sections chosen, as the first and last of their places in reading order, after one is ticked.
/// Ticking one outside the run reaches the run out to it, so the ones between are chosen too.
/// Ticking the run's first or last takes it out; ticking one inside ends the run there.
pub fn ticked(run: Option<(usize, usize)>, at: usize) -> Option<(usize, usize)> {
    let Some((first, last)) = run else {
        return Some((at, at));
    };

    if at < first {
        Some((at, last))
    } else if at > last {
        Some((first, at))
    } else if first == last {
        None
    } else if at == first {
        Some((first + 1, last))
    } else if at == last {
        Some((first, last - 1))
    } else {
        Some((first, at))
    }
}

/// How a copy's time is moved later: left as it is, by so many days, to no end, or until a later day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extending {
    AsItIs,
    Days(i64),
    NoEnd,
    Until(Date),
}

/// A copy's times once extended, which only ever move later.
/// Days are counted from where a time stands now, or from now if it has passed; a day earlier than where it stands is not taken.
/// A time with no end stays so, and the copy still never stays open beyond its deletion.
pub fn extended(
    now: OffsetDateTime,
    here: UtcOffset,
    (open_until, deleted_on): (Option<OffsetDateTime>, Option<OffsetDateTime>),
    open: Extending,
    kept: Extending,
) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    let later = |stands: Option<OffsetDateTime>, moved: Extending| match (stands, moved) {
        (None, _) => None,
        (stands, Extending::AsItIs) => stands,
        (Some(stands), Extending::Days(days)) => Some(stands.max(now) + Duration::days(days)),
        (Some(_), Extending::NoEnd) => None,
        (Some(stands), Extending::Until(day)) => Some(end_of(day, here).max(stands)),
    };
    let deleted_on = later(deleted_on, kept);
    let open_until = match (later(open_until, open), deleted_on) {
        (Some(open), Some(deleted)) => Some(open.min(deleted)),
        (None, deleted) => deleted,
        (open, None) => open,
    };

    (open_until, deleted_on)
}

#[cfg(test)]
mod tests {
    use time::macros::{date, datetime};

    use super::*;

    const NOW: OffsetDateTime = datetime!(2026-11-18 10:00 UTC);

    #[test]
    fn a_copy_closes_and_is_deleted_as_many_days_on_as_chosen() {
        // when: a copy is open for 30 days and kept for 90
        let (open_until, deleted_on) =
            times(NOW, UtcOffset::UTC, OpenFor::Days(30), KeptFor::Days(90));

        // then: it closes and is deleted that many days from now
        assert_eq!(open_until, Some(NOW + Duration::days(30)));
        assert_eq!(deleted_on, Some(NOW + Duration::days(90)));
    }

    #[test]
    fn a_copy_never_stays_open_beyond_its_deletion() {
        // when: a copy is chosen open for 90 days, or with no end, but kept for 30
        let longer = times(NOW, UtcOffset::UTC, OpenFor::Days(90), KeptFor::Days(30));
        let endless = times(NOW, UtcOffset::UTC, OpenFor::NoEnd, KeptFor::Days(30));

        // then: it closes when it is deleted
        for (open_until, deleted_on) in [longer, endless] {
            assert_eq!(open_until, deleted_on);
            assert_eq!(deleted_on, Some(NOW + Duration::days(30)));
        }
    }

    #[test]
    fn a_copy_kept_for_good_may_stay_open_with_no_end() {
        // expect: open with no end and kept for good, it has neither moment
        assert_eq!(
            times(NOW, UtcOffset::UTC, OpenFor::NoEnd, KeptFor::Ever),
            (None, None)
        );
    }

    #[test]
    fn a_day_chosen_lasts_until_its_end_where_the_reader_is() {
        // when: a copy is open until a day, for a reader two hours east
        let here = UtcOffset::from_hms(2, 0, 0).expect("an offset");
        let (open_until, _) = times(
            NOW,
            here,
            OpenFor::Until(date!(2026 - 12 - 17)),
            KeptFor::Ever,
        );

        // then: it is open to the last moment of that day there
        assert_eq!(open_until, Some(datetime!(2026-12-17 23:59:59 +2)));
    }

    #[test]
    fn ticking_a_section_further_away_takes_in_the_ones_between() {
        // given: one section ticked
        let run = ticked(None, 3);
        assert_eq!(run, Some((3, 3)));

        // when: one further on, and then one before, are ticked
        let further = ticked(run, 6);
        let before = ticked(further, 1);

        // then: the run reaches out to each
        assert_eq!(further, Some((3, 6)));
        assert_eq!(before, Some((1, 6)));
    }

    #[test]
    fn ticking_the_run_s_end_takes_it_out_and_ticking_inside_ends_the_run_there() {
        // given: a run of sections
        let run = Some((2, 6));

        // expect: its first and last are taken out, one inside ends it, and the last one left leaves none
        assert_eq!(ticked(run, 2), Some((3, 6)));
        assert_eq!(ticked(run, 6), Some((2, 5)));
        assert_eq!(ticked(run, 4), Some((2, 4)));
        assert_eq!(ticked(Some((4, 4)), 4), None);
    }

    #[test]
    fn days_are_counted_on_from_where_a_time_stands_or_from_now_once_it_has_passed() {
        // given: a copy open for ten more days, and one closed ten days ago, each deleted in sixty
        let deleted = Some(NOW + Duration::days(60));
        let open = (Some(NOW + Duration::days(10)), deleted);
        let closed = (Some(NOW - Duration::days(10)), deleted);

        // when: each is opened for 30 days more
        let (still_open, _) = extended(
            NOW,
            UtcOffset::UTC,
            open,
            Extending::Days(30),
            Extending::AsItIs,
        );
        let (open_again, _) = extended(
            NOW,
            UtcOffset::UTC,
            closed,
            Extending::Days(30),
            Extending::AsItIs,
        );

        // then: the open one runs on from its end, the closed one from now
        assert_eq!(still_open, Some(NOW + Duration::days(40)));
        assert_eq!(open_again, Some(NOW + Duration::days(30)));
    }

    #[test]
    fn a_day_earlier_than_where_a_time_stands_is_not_taken() {
        // given: a copy deleted in sixty days
        let stands = (
            Some(NOW + Duration::days(10)),
            Some(NOW + Duration::days(60)),
        );

        // when: it is to be deleted on a day before that
        let (_, deleted_on) = extended(
            NOW,
            UtcOffset::UTC,
            stands,
            Extending::AsItIs,
            Extending::Until(date!(2026 - 11 - 20)),
        );

        // then: it is deleted when it was to be, not earlier
        assert_eq!(deleted_on, Some(NOW + Duration::days(60)));
    }

    #[test]
    fn a_time_with_no_end_stays_so() {
        // given: a copy open with no end and kept for good
        let endless = (None, None);

        // when: it is given an end
        let moved = extended(
            NOW,
            UtcOffset::UTC,
            endless,
            Extending::Days(7),
            Extending::Days(90),
        );

        // then: it keeps having none, as an end would cut it short
        assert_eq!(moved, (None, None));
    }

    #[test]
    fn an_extended_copy_still_never_stays_open_beyond_its_deletion() {
        // given: a copy open for ten more days and deleted in twenty
        let stands = (
            Some(NOW + Duration::days(10)),
            Some(NOW + Duration::days(20)),
        );

        // when: it is opened with no end, its deletion left as it is
        let (open_until, deleted_on) = extended(
            NOW,
            UtcOffset::UTC,
            stands,
            Extending::NoEnd,
            Extending::AsItIs,
        );

        // then: it closes when it is deleted
        assert_eq!(open_until, deleted_on);
        assert_eq!(deleted_on, Some(NOW + Duration::days(20)));
    }
}
