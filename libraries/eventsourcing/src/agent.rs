use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Agent {
    Local,
    System,
    Account(AccountId),
}

impl AccountId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AccountId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for AccountId {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Display for Agent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => f.write_str("local"),
            Self::System => f.write_str("system"),
            Self::Account(id) => write!(f, "account {id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_agent_carries_who_it_is() {
        let acting = Agent::Account(AccountId::from("author-7"));

        assert_eq!(acting, Agent::Account(AccountId::from("author-7")));
        assert_ne!(acting, Agent::Account(AccountId::from("author-8")));
    }

    #[test]
    fn the_placeless_agents_need_no_id() {
        assert_eq!(Agent::System, Agent::System);
        assert_eq!(Agent::Local, Agent::Local);
        assert_ne!(Agent::System, Agent::Local);
    }

    #[test]
    fn an_agent_reads_well_in_an_audit_line() {
        assert_eq!(Agent::Local.to_string(), "local");
        assert_eq!(Agent::System.to_string(), "system");
        assert_eq!(
            Agent::Account(AccountId::from("author-7")).to_string(),
            "account author-7"
        );
    }
}
