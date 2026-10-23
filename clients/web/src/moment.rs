use fluent_bundle::FluentArgs;
use time::{OffsetDateTime, UtcOffset};

use crate::speech;

pub fn human_time(moment: OffsetDateTime) -> String {
    let here = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);

    told(moment.to_offset(here))
}

fn told(moment: OffsetDateTime) -> String {
    let mut args = FluentArgs::new();
    args.set("day", moment.day());
    args.set("month", u8::from(moment.month()));
    args.set("year", moment.year().to_string());
    args.set("hour", format!("{:02}", moment.hour()));
    args.set("minute", format!("{:02}", moment.minute()));

    speech::say_with("moment", &args)
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn a_moment_is_told_by_its_day_month_year_and_time() {
        assert_eq!(told(datetime!(2026-10-10 09:01 UTC)), "10 Oct 2026, 09:01");
    }

    #[test]
    fn the_first_and_last_months_are_named_and_the_day_is_not_padded() {
        assert_eq!(told(datetime!(2026-01-05 23:59 UTC)), "5 Jan 2026, 23:59");
        assert_eq!(told(datetime!(2026-12-31 00:00 UTC)), "31 Dec 2026, 00:00");
    }
}
