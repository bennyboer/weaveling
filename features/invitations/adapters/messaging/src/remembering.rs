use accounts_contract::{AccountEventDTO, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use async_trait::async_trait;
use email::EmailAddress;
use eventsourcing::AccountId;
use inbox::{KeptHandler, Refused};
use invitations_core::{InvitationsCore, Person};
use messaging::Message;
use transacting::Transaction;

pub struct RememberWhoSignedUpListener {
    invitations: InvitationsCore,
}

impl RememberWhoSignedUpListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl KeptHandler for RememberWhoSignedUpListener {
    const NAME: &'static str = "invitations-remember-who-signed-up";
    const LISTENS_TO: &'static [&'static str] = &[SIGNED_UP, SIGNED_UP_BY_INVITATION];
    const WHEN_REFUSED: &'static str = "Someone who signed up is not known to invitations, so invitations to their address and from them cannot say whose they are.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let (AccountEventDTO::SignedUp { personal }
        | AccountEventDTO::SignedUpByInvitation { personal }) = told.event.body
        else {
            return Err("this is no sign-up".into());
        };

        let person = Person {
            account: AccountId::from(told.aggregate.id),
            name: personal.name,
            email: EmailAddress::new(&personal.email)?,
        };

        self.invitations
            .remember_person(Some(transaction), &person)
            .await
            .map_err(Refused::of)
    }
}
