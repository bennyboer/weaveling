use fluent_bundle::FluentArgs;

use crate::speech::catalogue;
use crate::speech::language::Language;

/// The messages a size is told in, from bytes to terabytes.
/// Each unit is 1024 times the one before, as the settings count them.
const UNITS: [&str; 5] = [
    "size-bytes",
    "size-kilobytes",
    "size-megabytes",
    "size-gigabytes",
    "size-terabytes",
];

/// A size as it is shown, for example 1.5 MB.
/// `whole` is the part before the decimal point (1), and `decimal` the one digit after it (5), if any.
/// `unit` is the index of the unit in `UNITS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scaled {
    whole: u64,
    decimal: Option<u64>,
    unit: usize,
}

/// Scales a size in bytes to the largest unit it reaches.
/// Below 10 of a unit, the amount keeps one digit after the decimal point. From 10 on, it is rounded to a whole number.
/// A digit of zero after the point is left out, so 1.0 MB is shown as 1 MB.
fn scaled(bytes: u64) -> Scaled {
    let bytes = u128::from(bytes);
    let mut unit = 0;
    while unit + 1 < UNITS.len() && bytes >= 1024_u128.pow(unit as u32 + 1) {
        unit += 1;
    }
    if unit == 0 {
        return Scaled {
            whole: bytes as u64,
            decimal: None,
            unit,
        };
    }

    let size = 1024_u128.pow(unit as u32);
    let in_tenths = (bytes * 10 + size / 2) / size;
    if in_tenths < 100 {
        let decimal = (in_tenths % 10) as u64;
        return Scaled {
            whole: (in_tenths / 10) as u64,
            decimal: (decimal > 0).then_some(decimal),
            unit,
        };
    }

    let whole = (bytes + size / 2) / size;
    if whole >= 1024 && unit + 1 < UNITS.len() {
        return Scaled {
            whole: 1,
            decimal: None,
            unit: unit + 1,
        };
    }

    Scaled {
        whole: whole as u64,
        decimal: None,
        unit,
    }
}

/// Tells a size in bytes in the given language, for example "1.4 MB" or "1,4 Mo".
pub fn told(bytes: u64, language: Language) -> String {
    let Scaled {
        whole,
        decimal,
        unit,
    } = scaled(bytes);
    let amount = match decimal {
        Some(decimal) => format!(
            "{whole}{}{decimal}",
            catalogue::worded(language, "decimal-point", None)
        ),
        None => whole.to_string(),
    };
    let mut args = FluentArgs::new();
    args.set("amount", amount);

    catalogue::worded(language, UNITS[unit], Some(&args))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    fn shown(bytes: u64) -> (u64, Option<u64>, &'static str) {
        let scaled = scaled(bytes);

        (scaled.whole, scaled.decimal, UNITS[scaled.unit])
    }

    #[test]
    fn a_size_below_a_kilobyte_is_told_in_bytes() {
        // expect: nothing and a few bytes, as they are
        assert_eq!(shown(0), (0, None, "size-bytes"));
        assert_eq!(shown(1023), (1023, None, "size-bytes"));
    }

    #[test]
    fn a_size_below_ten_of_its_unit_keeps_one_decimal() {
        // expect: a decimal where there is one
        assert_eq!(shown(KB), (1, None, "size-kilobytes"));
        assert_eq!(shown(1536 * KB), (1, Some(5), "size-megabytes"));
        assert_eq!(shown(9 * GB + GB / 2), (9, Some(5), "size-gigabytes"));
    }

    #[test]
    fn a_size_of_ten_or_more_of_its_unit_is_rounded_to_a_whole() {
        // expect: no decimal from ten on
        assert_eq!(shown(10 * MB), (10, None, "size-megabytes"));
        assert_eq!(shown(20 * MB + MB / 2), (21, None, "size-megabytes"));
        assert_eq!(shown(500 * MB), (500, None, "size-megabytes"));
    }

    #[test]
    fn a_size_rounded_up_to_the_next_unit_is_told_in_that_unit() {
        // expect: just under a megabyte, rounded, is one megabyte
        assert_eq!(shown(MB - 1), (1, None, "size-megabytes"));
        // and: 9.96 MB, rounded up to 10, is shown without a decimal
        assert_eq!(shown(10 * MB - 1), (10, None, "size-megabytes"));
    }

    #[test]
    fn a_size_is_told_with_the_language_s_decimal_point_and_unit() {
        // expect: english words a size with a point
        assert_eq!(told(1536 * KB, Language::English), "1.5\u{a0}MB");
        assert_eq!(told(20 * MB, Language::English), "20\u{a0}MB");
        assert_eq!(told(512, Language::English), "512\u{a0}B");
    }

    #[test]
    fn a_limit_reached_tells_its_sizes_as_sizes() {
        // given: an account's limit reached, its sizes told
        let mut args = FluentArgs::new();
        args.set("bound", "account");
        args.set("used", told(1536 * KB, Language::English));
        args.set("most", told(2 * MB, Language::English));

        // expect: the refusal names both sizes
        assert_eq!(
            catalogue::worded(
                Language::English,
                "refused-asset-limit-reached",
                Some(&args)
            ),
            "You have no room left for this image. You use 1.5\u{a0}MB of 2\u{a0}MB."
        );
    }
}
