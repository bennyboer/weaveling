use time::OffsetDateTime;

use email::EmailAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationTicket {
    pub invitation: String,
    pub to: EmailAddress,
    pub until: OffsetDateTime,
}
