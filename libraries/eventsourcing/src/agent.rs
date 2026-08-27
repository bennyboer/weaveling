use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Agent {
    Anonymous,
    System,
    User(AgentId),
}

impl AgentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AgentId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for AgentId {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for AgentId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Display for Agent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anonymous => f.write_str("anonymous"),
            Self::System => f.write_str("system"),
            Self::User(id) => write!(f, "user {id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_user_agent_carries_who_it_is() {
        let acting = Agent::User(AgentId::from("author-7"));

        assert_eq!(acting, Agent::User(AgentId::from("author-7")));
        assert_ne!(acting, Agent::User(AgentId::from("author-8")));
    }

    #[test]
    fn the_placeless_agents_need_no_id() {
        assert_eq!(Agent::System, Agent::System);
        assert_eq!(Agent::Anonymous, Agent::Anonymous);
        assert_ne!(Agent::System, Agent::Anonymous);
    }

    #[test]
    fn an_agent_reads_well_in_an_audit_line() {
        assert_eq!(Agent::Anonymous.to_string(), "anonymous");
        assert_eq!(Agent::System.to_string(), "system");
        assert_eq!(
            Agent::User(AgentId::from("author-7")).to_string(),
            "user author-7"
        );
    }
}
