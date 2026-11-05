use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use authorizing::AllowingEverything;
use invitations_catalog::InMemoryInvitationCatalog;
use invitations_contract::ISSUED;
use invitations_core::{InvitationId, InvitationToken, InvitationsCore, TokenKeys};
use invitations_messaging::MailIssuedInvitationListener;
use mailing::{FakeMailbox, Mail, Mailer, Unsent};
use messaging::{Listener, Message};
use outbox::Outbox;
use throttling::InMemoryThrottles;
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

#[derive(Default)]
struct DownOnce {
    tried: AtomicBool,
    mailbox: FakeMailbox,
}

#[async_trait]
impl Mailer for DownOnce {
    async fn send(&self, mail: Mail) -> Result<(), Unsent> {
        if !self.tried.swap(true, Ordering::AcqRel) {
            return Err(Unsent {
                to: mail.to,
                because: "the mail server is down".to_owned(),
            });
        }

        self.mailbox.send(mail).await
    }
}

async fn an_open_invitation(wired: &Wired) -> InvitationId {
    wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id
}

async fn the_issue_told(wired: &Wired) -> Message {
    wired
        .events
        .deliver(100)
        .await
        .expect("delivering should succeed");

    wired
        .told
        .heard()
        .into_iter()
        .find(|told| told.routing.to_string() == ISSUED)
        .expect("an issue is told")
}

#[tokio::test]
async fn an_issue_mails_nothing_until_it_is_heard_and_then_the_link_derived_for_it() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;
    // then: nothing is mailed yet
    assert!(
        wired.mailbox.sent_to(ADA).is_empty(),
        "the mail is the listener's, after the issue has committed"
    );

    // when: its issue is heard
    let token = wired.token_mailed_to(ADA).await;

    // then: the token mailed is the one derived for it
    assert_eq!(
        token.as_str(),
        InvitationToken::derived(&wired.keys.key().await.expect("a key is at hand"), &id).as_str(),
        "the token mailed is the one derived for the invitation, so a mail heard again carries it too"
    );
}

#[tokio::test]
async fn an_issue_heard_twice_is_mailed_once() {
    // given: the issue of an invitation told
    let wired = wiring::a_workbench();
    an_open_invitation(&wired).await;
    let issued = the_issue_told(&wired).await;

    // when: it is heard twice
    for _ in 0..2 {
        wired
            .mailing
            .handle(&issued)
            .await
            .expect("messages come at least once");
    }

    // then: it is mailed once
    assert_eq!(wired.mailbox.sent_to(ADA).len(), 1);
}

#[tokio::test]
async fn a_mail_that_could_not_be_sent_is_refused_and_sent_when_the_issue_is_offered_again() {
    // given: a mail server down once
    let wired = wiring::a_workbench();
    let down_once = Arc::new(DownOnce::default());
    let mailing = wiring::inboxed_effect(MailIssuedInvitationListener::new(InvitationsCore::new(
        wired.store.clone(),
        Arc::new(InMemoryTransactions::new()),
        wired.keys.clone(),
        Arc::new(InMemoryThrottles::default()),
        Arc::new(InMemoryInvitationCatalog::new()),
        down_once.clone(),
        Arc::new(AllowingEverything),
        wired.clock.clone(),
    )));
    // and: the issue of an invitation told
    let id = an_open_invitation(&wired).await;
    let issued = the_issue_told(&wired).await;

    // when: the issue is heard
    let refused = mailing.handle(&issued).await;
    // and: heard again
    mailing
        .handle(&issued)
        .await
        .expect("the mail server is up again");

    // then: the first hearing is refused
    assert!(refused.is_err(), "a mail not sent is no issue heard");
    // and: the link the invitation was issued with is mailed once
    let mailed = down_once.mailbox.sent_to(ADA);
    assert_eq!(mailed.len(), 1);
    assert!(
        mailed[0].body.contains(
            InvitationToken::derived(&wired.keys.key().await.expect("a key is at hand"), &id)
                .as_str()
        ),
        "the link mailed late is the link the invitation was issued with"
    );
}

#[tokio::test]
async fn an_invitation_withdrawn_before_its_mail_was_heard_is_not_mailed() {
    // given: an invitation withdrawn before its mail was heard
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;
    wired
        .invitations
        .withdraw(&wiring::an_operator(), &id, None)
        .await
        .expect("an operator may withdraw");

    // then: nothing is mailed
    assert!(
        wired.sent_to(ADA).await.is_empty(),
        "a link mailed now would lead to an invitation already gone"
    );
}
