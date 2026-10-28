use invitations_core::{CatalogError, DeclineReason, Outcome};

const OPEN: &str = "open";
const ACCEPTED: &str = "accepted";
const WITHDRAWN: &str = "withdrawn";
const DECLINED: &str = "declined";
const ADDRESS_HAS_ACCOUNT: &str = "address-has-account";
const TURNED_DOWN: &str = "turned-down";

pub fn written(outcome: Outcome) -> (&'static str, Option<&'static str>) {
    match outcome {
        Outcome::Open => (OPEN, None),
        Outcome::Accepted => (ACCEPTED, None),
        Outcome::Withdrawn => (WITHDRAWN, None),
        Outcome::Declined(DeclineReason::AddressHasAccount) => {
            (DECLINED, Some(ADDRESS_HAS_ACCOUNT))
        }
        Outcome::Declined(DeclineReason::TurnedDown) => (DECLINED, Some(TURNED_DOWN)),
    }
}

pub fn read(outcome: &str, declined_for: Option<&str>) -> Result<Outcome, CatalogError> {
    match (outcome, declined_for) {
        (OPEN, None) => Ok(Outcome::Open),
        (ACCEPTED, None) => Ok(Outcome::Accepted),
        (WITHDRAWN, None) => Ok(Outcome::Withdrawn),
        (DECLINED, Some(ADDRESS_HAS_ACCOUNT)) => {
            Ok(Outcome::Declined(DeclineReason::AddressHasAccount))
        }
        (DECLINED, Some(TURNED_DOWN)) => Ok(Outcome::Declined(DeclineReason::TurnedDown)),
        _ => Err(CatalogError::Backend(
            format!("{outcome:?} for {declined_for:?} is no way for an invitation to stand").into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_outcome_reads_back_as_it_was_written() {
        for outcome in [
            Outcome::Open,
            Outcome::Accepted,
            Outcome::Withdrawn,
            Outcome::Declined(DeclineReason::AddressHasAccount),
            Outcome::Declined(DeclineReason::TurnedDown),
        ] {
            let (kept, declined_for) = written(outcome);

            assert_eq!(read(kept, declined_for).expect("readable"), outcome);
        }
    }

    #[test]
    fn a_way_to_stand_nobody_wrote_is_refused() {
        for (outcome, declined_for) in [
            ("pending", None),
            ("declined", None),
            ("declined", Some("had a bad day")),
            ("open", Some("address-has-account")),
            ("open", Some("turned-down")),
        ] {
            assert!(
                read(outcome, declined_for).is_err(),
                "{outcome} {declined_for:?}"
            );
        }
    }
}
