use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use acting::Acting;
use authorizing::{AllowingEverything, Authorizer};
use clock::FixedClock;
use eventsourcing::{Agent, InMemoryEventStore};
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use invitations_catalog::{InMemoryInvitationCatalog, InMemoryTokenKeys};
use invitations_contract::ISSUED;
use invitations_core::{InvitationEvent, InvitationToken, InvitationsCore};
use invitations_messaging::MailIssuedInvitationListener;
use mailing::{FakeMailbox, Mail};
use messaging::{FakePublisher, Listener, Message};
use outbox::{InMemoryOutbox, Outbox};
use throttling::InMemoryThrottles;
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;

pub const ORIGIN: &str = "https://weaveling.example.com";

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

pub struct Wired {
    pub invitations: InvitationsCore,
    pub store: Arc<InMemoryEventStore<InvitationEvent>>,
    pub catalog: Arc<InMemoryInvitationCatalog>,
    pub mailbox: Arc<FakeMailbox>,
    pub clock: Arc<FixedClock>,
    pub outbox: Arc<InMemoryOutbox>,
    pub published: Arc<FakePublisher>,
    pub keys: Arc<InMemoryTokenKeys>,
    pub events: Arc<InMemoryOutbox>,
    pub told: Arc<FakePublisher>,
    pub mailing: Inboxed<MailIssuedInvitationListener>,
    handed: AtomicUsize,
}

pub fn wired() -> Wired {
    wired_guarded_by(Arc::new(AllowingEverything))
}

pub fn wired_guarded_by(authorizer: Arc<dyn Authorizer>) -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let told = Arc::new(FakePublisher::default());
    let events = Arc::new(InMemoryOutbox::new(told.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        events.clone(),
        invitations_messaging::message_for,
    ));
    let keys = Arc::new(InMemoryTokenKeys::new());
    let mailbox = Arc::new(FakeMailbox::default());
    let catalog = Arc::new(InMemoryInvitationCatalog::new());
    let published = Arc::new(FakePublisher::default());
    let outbox = Arc::new(InMemoryOutbox::new(published.clone(), clock.clone()));

    let invitations = InvitationsCore::new(
        store.clone(),
        Arc::new(InMemoryTransactions::new()),
        keys.clone(),
        Arc::new(InMemoryThrottles::default()),
        catalog.clone(),
        mailbox.clone(),
        authorizer,
        clock.clone(),
    );

    Wired {
        mailing: inboxed(MailIssuedInvitationListener::new(invitations.clone())),
        invitations,
        store,
        catalog,
        mailbox,
        clock,
        outbox,
        published,
        keys,
        events,
        told,
        handed: AtomicUsize::new(0),
    }
}

pub fn inboxed<H: KeptHandler>(handler: H) -> Inboxed<H> {
    inboxed_in(handler, InMemoryTransactions::new())
}

pub fn inboxed_in<H: KeptHandler>(handler: H, hearings: InMemoryTransactions) -> Inboxed<H> {
    Inboxed::new(
        handler,
        Arc::new(hearings),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(at(
            1_700_000_000,
        ))))),
    )
}

pub fn an_operator() -> Acting {
    Acting {
        agent: Agent::Account("account_grace".into()),
        operator: true,
    }
}

pub fn no_operator() -> Acting {
    Acting {
        agent: Agent::Account("account_ada".into()),
        operator: false,
    }
}

impl Wired {
    pub async fn follow_ups(&self) -> Vec<Message> {
        self.outbox
            .deliver(100)
            .await
            .expect("delivering should succeed");

        self.published.heard()
    }

    pub async fn followed_up_by(&self, listeners: &[&dyn Listener]) {
        let mut handed = 0;
        loop {
            let follow_ups = self.follow_ups().await;
            if follow_ups.len() == handed {
                return;
            }
            for follow_up in &follow_ups[handed..] {
                for listener in listeners {
                    if listener.hears(&follow_up.routing) {
                        listener
                            .handle(follow_up)
                            .await
                            .expect("a follow-up is taken in");
                    }
                }
            }
            handed = follow_ups.len();
        }
    }

    pub async fn mailed(&self) {
        self.events
            .deliver(100)
            .await
            .expect("delivering should succeed");
        let told = self.told.heard();
        for issued in &told[self.handed.swap(told.len(), Ordering::AcqRel)..] {
            if issued.routing.to_string() == ISSUED {
                self.mailing
                    .handle(issued)
                    .await
                    .expect("an issued invitation is mailed");
            }
        }
    }

    pub async fn sent_to(&self, address: &str) -> Vec<Mail> {
        self.mailed().await;

        self.mailbox.sent_to(address)
    }

    pub async fn links_mailed_to(&self, address: &str) -> Vec<String> {
        self.sent_to(address)
            .await
            .iter()
            .filter_map(|mail| {
                mail.body
                    .split_whitespace()
                    .find(|word| word.contains("/invitation#"))
                    .map(str::to_owned)
            })
            .collect()
    }

    pub async fn token_mailed_to(&self, address: &str) -> InvitationToken {
        let link = self
            .links_mailed_to(address)
            .await
            .pop()
            .expect("a link should have been mailed");

        InvitationToken::presented(
            link.split_once("/invitation#")
                .expect("the link carries its token")
                .1,
        )
    }
}
