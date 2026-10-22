use crate::agent::{AccountId, Agent};

const LOCAL: &str = "local";
const SYSTEM: &str = "system";
const ACCOUNT_PREFIX: &str = "account:";

pub(crate) fn encode(agent: &Agent) -> String {
    match agent {
        Agent::Local => LOCAL.to_owned(),
        Agent::System => SYSTEM.to_owned(),
        Agent::Account(id) => format!("{ACCOUNT_PREFIX}{id}"),
    }
}

pub(crate) fn decode(stored: &str) -> Agent {
    match stored {
        SYSTEM => Agent::System,
        other => match other.strip_prefix(ACCOUNT_PREFIX) {
            Some(id) => Agent::Account(AccountId::from(id)),
            None => Agent::Local,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(agent: Agent) -> Agent {
        decode(&encode(&agent))
    }

    #[test]
    fn every_kind_of_agent_survives_a_round_trip() {
        assert_eq!(round_trip(Agent::Local), Agent::Local);
        assert_eq!(round_trip(Agent::System), Agent::System);
        assert_eq!(
            round_trip(Agent::Account(AccountId::from("author-7"))),
            Agent::Account(AccountId::from("author-7"))
        );
    }

    #[test]
    fn an_agent_is_written_as_something_a_person_can_read_in_the_table() {
        assert_eq!(encode(&Agent::Local), "local");
        assert_eq!(encode(&Agent::System), "system");
        assert_eq!(
            encode(&Agent::Account(AccountId::from("author-7"))),
            "account:author-7"
        );
    }

    #[test]
    fn an_account_named_like_the_others_is_still_an_account() {
        assert_eq!(
            round_trip(Agent::Account(AccountId::from("system"))),
            Agent::Account(AccountId::from("system"))
        );
    }

    #[test]
    fn text_from_nowhere_names_nobody() {
        assert_eq!(decode("scribbled by hand"), Agent::Local);
    }

    #[test]
    fn what_was_written_as_anonymous_before_accounts_reads_as_local() {
        assert_eq!(decode("anonymous"), Agent::Local);
    }
}
