use throttling::Rule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Guarded {
    SigningIn,
    SendingCodes,
    CheckingSecondFactor,
}

impl Guarded {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SigningIn => "signing-in",
            Self::SendingCodes => "sending-codes",
            Self::CheckingSecondFactor => "checking-second-factor",
        }
    }

    pub fn rule(self) -> Rule {
        match self {
            Self::SigningIn => Rule { free: 5 },
            Self::SendingCodes => Rule { free: 3 },
            Self::CheckingSecondFactor => Rule { free: 5 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_in_forgives_more_than_sending_codes() {
        // expect: signing in is free five times
        assert_eq!(Guarded::SigningIn.rule(), Rule { free: 5 });
        // and: sending codes three times
        assert_eq!(
            Guarded::SendingCodes.rule(),
            Rule { free: 3 },
            "a mistyped password costs nobody anything, a code sent costs a stranger a mail"
        );
    }

    #[test]
    fn each_guarded_thing_is_counted_under_a_name_of_its_own() {
        // expect: signing in and sending codes have different names
        let names = [
            Guarded::SigningIn.as_str(),
            Guarded::SendingCodes.as_str(),
            Guarded::CheckingSecondFactor.as_str(),
        ];
        assert!(
            names
                .iter()
                .enumerate()
                .all(|(at, name)| !names[..at].contains(name))
        );
    }
}
