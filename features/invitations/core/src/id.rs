use eventsourcing::AggregateId;

ids::id!(InvitationId, "invitation_");

impl From<&InvitationId> for AggregateId {
    fn from(id: &InvitationId) -> Self {
        AggregateId::from(id.to_string())
    }
}
