use crate::agent::{Agent, AgentId};

const ANONYMOUS: &str = "anonymous";
const SYSTEM: &str = "system";
const USER_PREFIX: &str = "user:";

pub(crate) fn encode(agent: &Agent) -> String {
    match agent {
        Agent::Anonymous => ANONYMOUS.to_owned(),
        Agent::System => SYSTEM.to_owned(),
        Agent::User(id) => format!("{USER_PREFIX}{id}"),
    }
}

pub(crate) fn decode(stored: &str) -> Agent {
    match stored {
        SYSTEM => Agent::System,
        other => match other.strip_prefix(USER_PREFIX) {
            Some(id) => Agent::User(AgentId::from(id)),
            None => Agent::Anonymous,
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
        assert_eq!(round_trip(Agent::Anonymous), Agent::Anonymous);
        assert_eq!(round_trip(Agent::System), Agent::System);
        assert_eq!(
            round_trip(Agent::User(AgentId::from("author-7"))),
            Agent::User(AgentId::from("author-7"))
        );
    }

    #[test]
    fn an_agent_is_written_as_something_a_person_can_read_in_the_table() {
        assert_eq!(encode(&Agent::Anonymous), "anonymous");
        assert_eq!(encode(&Agent::System), "system");
        assert_eq!(
            encode(&Agent::User(AgentId::from("author-7"))),
            "user:author-7"
        );
    }

    #[test]
    fn a_user_named_like_the_others_is_still_a_user() {
        assert_eq!(
            round_trip(Agent::User(AgentId::from("system"))),
            Agent::User(AgentId::from("system"))
        );
    }

    #[test]
    fn text_from_nowhere_names_nobody() {
        assert_eq!(decode("scribbled by hand"), Agent::Anonymous);
    }
}
