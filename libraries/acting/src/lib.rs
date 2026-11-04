use eventsourcing::Agent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acting {
    pub agent: Agent,
    pub operator: bool,
}
