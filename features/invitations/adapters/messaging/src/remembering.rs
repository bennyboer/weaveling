use accounts_contract::{AccountEventDTO, DELETED, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use async_trait::async_trait;
use email::EmailAddress;
use eventsourcing::AccountId;
use inbox::KeptHandler;
use invitations_core::{InvitationsCore, Person};
use messaging::{Failure, Message};
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
    const WHEN_REFUSED: &'static str = "The invitations feature did not learn about a new sign-up. Invitations sent to this person's address, or sent by them, cannot show whose they are.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
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

        Ok(self
            .invitations
            .remember_person(Some(transaction), &person)
            .await?)
    }
}

pub struct ForgetDeletedAccountListener {
    invitations: InvitationsCore,
}

impl ForgetDeletedAccountListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl KeptHandler for ForgetDeletedAccountListener {
    const NAME: &'static str = "invitations-forget-deleted-account";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str =
        "The invitations feature still stores the name and address of a deleted account.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let AccountEventDTO::Deleted = told.event.body else {
            return Err("this tells no account deleted".into());
        };

        Ok(self
            .invitations
            .forget_person(Some(transaction), &AccountId::from(told.aggregate.id))
            .await?)
    }
}
