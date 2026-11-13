use thiserror::Error;

const KILOBYTE: u128 = 1024;
const MOST_DECIMALS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UnreadableSize {
    #[error("no number; write one with a unit, like 20MB")]
    NoNumber,
    #[error("no unit; add one, like 20MB, as a number alone is not taken for bytes")]
    NoUnit,
    #[error("a comma; write a point, like 1.5GB")]
    Comma,
    #[error("unknown unit {0}; take B, KB, MB, GB or TB")]
    UnknownUnit(String),
    #[error("more than can be counted")]
    TooLarge,
}

/// Reads a written size as a number of bytes.
///
/// A size is a number, whole or with a decimal point, followed by its unit: B, KB, MB, GB or TB.
/// The unit may be in any case, with or without a space before it.
/// A kilobyte is 1024 bytes, so KiB, MiB, GiB and TiB mean the same as KB, MB, GB and TB.
/// A fraction of a byte is dropped.
pub fn bytes(written: &str) -> Result<u64, UnreadableSize> {
    let written = written.trim();
    let (whole, rest) = digits(written);
    if whole.is_empty() {
        return Err(UnreadableSize::NoNumber);
    }
    if rest.starts_with(',') {
        return Err(UnreadableSize::Comma);
    }
    let (decimals, unit) = match rest.strip_prefix('.') {
        Some(after) => match digits(after) {
            ("", _) => return Err(UnreadableSize::NoNumber),
            read => read,
        },
        None => ("", rest),
    };
    let unit = unit.trim();
    if unit.is_empty() {
        return Err(UnreadableSize::NoUnit);
    }

    let each = each(unit)?;
    let decimals = &decimals[..decimals.len().min(MOST_DECIMALS)];
    let whole: u128 = whole.parse().map_err(|_| UnreadableSize::TooLarge)?;
    let share = match decimals {
        "" => 0,
        _ => {
            decimals
                .parse::<u128>()
                .map_err(|_| UnreadableSize::TooLarge)?
                * each
                / 10u128.pow(decimals.len() as u32)
        }
    };
    whole
        .checked_mul(each)
        .and_then(|counted| counted.checked_add(share))
        .and_then(|counted| u64::try_from(counted).ok())
        .ok_or(UnreadableSize::TooLarge)
}

fn digits(written: &str) -> (&str, &str) {
    let cut = written
        .find(|sign: char| !sign.is_ascii_digit())
        .unwrap_or(written.len());

    written.split_at(cut)
}

fn each(unit: &str) -> Result<u128, UnreadableSize> {
    let power = match unit.to_ascii_lowercase().as_str() {
        "b" => 0,
        "kb" | "kib" => 1,
        "mb" | "mib" => 2,
        "gb" | "gib" => 3,
        "tb" | "tib" => 4,
        _ => return Err(UnreadableSize::UnknownUnit(unit.to_owned())),
    };

    Ok(KILOBYTE.pow(power))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_unit_is_a_power_of_1024_bytes() {
        // expect: from bytes to terabytes, each 1024 times the one before
        assert_eq!(bytes("7B"), Ok(7));
        assert_eq!(bytes("4096KB"), Ok(4 * 1024 * 1024));
        assert_eq!(bytes("1024MB"), Ok(1024 * 1024 * 1024));
        assert_eq!(bytes("1GB"), Ok(1024 * 1024 * 1024));
        assert_eq!(bytes("2TB"), Ok(2 * 1024 * 1024 * 1024 * 1024));
    }

    #[test]
    fn a_unit_is_read_in_any_case_with_a_space_or_none() {
        // expect: however the unit is written, the size is the same
        for written in ["1GB", "1gb", "1Gb", "1 GB", " 1 gb ", "1GiB", "1gib"] {
            assert_eq!(bytes(written), Ok(1024 * 1024 * 1024), "{written}");
        }
    }

    #[test]
    fn nought_is_a_size() {
        // expect: nothing is counted as nothing, for whoever reads it to refuse
        assert_eq!(bytes("0MB"), Ok(0));
    }

    #[test]
    fn a_number_alone_is_refused_rather_than_taken_for_bytes() {
        // expect: twenty is not twenty bytes
        assert_eq!(bytes("20"), Err(UnreadableSize::NoUnit));
        assert_eq!(bytes(" 20 "), Err(UnreadableSize::NoUnit));
    }

    #[test]
    fn a_unit_alone_or_nothing_is_no_size() {
        // expect: without a number there is nothing to count
        for written in ["", "  ", "MB", "-1MB"] {
            assert_eq!(bytes(written), Err(UnreadableSize::NoNumber), "{written}");
        }
    }

    #[test]
    fn a_share_is_read_after_a_point() {
        // expect: a share of a unit counted exactly, whatever the unit
        assert_eq!(bytes("1.5GB"), Ok(1536 * 1024 * 1024));
        assert_eq!(bytes("0.25 mb"), Ok(256 * 1024));
        assert_eq!(bytes("2.0TB"), Ok(2 * 1024 * 1024 * 1024 * 1024));
    }

    #[test]
    fn a_part_of_a_byte_is_let_go() {
        // expect: 1331.2 bytes are 1331, and half a byte none
        assert_eq!(bytes("1.3KB"), Ok(1331));
        assert_eq!(bytes("0.5B"), Ok(0));
        assert_eq!(
            bytes("1.000000000000000000000000001GB"),
            Ok(1024 * 1024 * 1024)
        );
    }

    #[test]
    fn a_comma_is_refused_rather_than_guessed() {
        // expect: a comma could part thousands or shares, so it is neither
        assert_eq!(bytes("1,5GB"), Err(UnreadableSize::Comma));
        assert_eq!(bytes("1,024MB"), Err(UnreadableSize::Comma));
    }

    #[test]
    fn a_point_without_digits_after_it_is_no_number() {
        // expect: a share must be written
        assert_eq!(bytes("1.GB"), Err(UnreadableSize::NoNumber));
        assert_eq!(bytes(".5GB"), Err(UnreadableSize::NoNumber));
    }

    #[test]
    fn an_unknown_unit_is_named() {
        // expect: the unit not understood is said back
        assert_eq!(
            bytes("20 megs"),
            Err(UnreadableSize::UnknownUnit("megs".to_owned()))
        );
        assert_eq!(
            bytes("20M"),
            Err(UnreadableSize::UnknownUnit("M".to_owned()))
        );
        assert_eq!(
            bytes("20MB!"),
            Err(UnreadableSize::UnknownUnit("MB!".to_owned()))
        );
    }

    #[test]
    fn more_than_can_be_counted_is_refused() {
        // expect: a number past what a u64 holds, alone or by its unit
        assert_eq!(
            bytes("99999999999999999999B"),
            Err(UnreadableSize::TooLarge)
        );
        assert_eq!(bytes("16777216TB"), Err(UnreadableSize::TooLarge));
        assert_eq!(bytes("16777215TB"), Ok(16_777_215 * 1024u64.pow(4)));
        assert_eq!(bytes("18446744073709551615B"), Ok(u64::MAX));
        assert_eq!(bytes("18446744073709551615.9B"), Ok(u64::MAX));
        assert_eq!(
            bytes("18446744073709551616B"),
            Err(UnreadableSize::TooLarge)
        );
        assert_eq!(bytes("17179869184.0GB"), Err(UnreadableSize::TooLarge));
    }
}
