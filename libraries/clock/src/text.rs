use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

pub fn written(at: OffsetDateTime) -> String {
    let at = at.to_offset(UtcOffset::UTC);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
        at.nanosecond()
    )
}

pub fn read(written: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(written, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    fn at(seconds: i64, nanoseconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds) + Duration::nanoseconds(nanoseconds)
    }

    #[test]
    fn an_instant_survives_being_written_to_the_nanosecond() {
        let then = at(1_759_000_000, 123_456_789);

        assert_eq!(read(&written(then)), Some(then));
    }

    #[test]
    fn an_instant_is_written_in_utc_whatever_offset_it_came_with() {
        let then = at(1_000, 0).to_offset(UtcOffset::from_hms(2, 0, 0).expect("a valid offset"));

        assert_eq!(written(then), "1970-01-01T00:16:40.000000000Z");
    }

    #[test]
    fn written_instants_sort_as_text_the_way_they_sort_in_time() {
        let earlier = written(at(1_000, 5));
        let later = written(at(1_000, 40_000_000));

        assert!(
            earlier < later,
            "SQLite compares these as strings, so a claim's expiry relies on the width being fixed"
        );
    }

    #[test]
    fn text_from_nowhere_is_no_instant() {
        assert_eq!(read("yesterday, roughly"), None);
    }
}
