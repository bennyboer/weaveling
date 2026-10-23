use throttling::Rule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Guarded {
    SigningIn,
    SendingCodes,
}

impl Guarded {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SigningIn => "signing-in",
            Self::SendingCodes => "sending-codes",
        }
    }

    pub fn rule(self) -> Rule {
        match self {
            Self::SigningIn => Rule { free: 5 },
            Self::SendingCodes => Rule { free: 3 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_in_forgives_more_than_sending_codes() {
        assert_eq!(Guarded::SigningIn.rule(), Rule { free: 5 });
        assert_eq!(
            Guarded::SendingCodes.rule(),
            Rule { free: 3 },
            "a mistyped password costs nobody anything, a code sent costs a stranger a mail"
        );
    }

    #[test]
    fn each_guarded_thing_is_counted_under_a_name_of_its_own() {
        assert_ne!(Guarded::SigningIn.as_str(), Guarded::SendingCodes.as_str());
    }
}
