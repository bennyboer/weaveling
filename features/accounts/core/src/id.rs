use eventsourcing::{Agent, AggregateId};

ids::id!(AccountId, "account_");

impl AccountId {
    pub fn acting(&self) -> Agent {
        Agent::Account(self.to_string().into())
    }
}

impl From<&AccountId> for AggregateId {
    fn from(id: &AccountId) -> Self {
        AggregateId::from(id.to_string())
    }
}
