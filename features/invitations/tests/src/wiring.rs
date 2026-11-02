use std::sync::Arc;

use acting::Acting;
use authorizing::{AllowingEverything, Authorizer};
use clock::FixedClock;
use eventsourcing::{Agent, InMemoryEventStore};
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use invitations_catalog::InMemoryInvitationCatalog;
use invitations_core::{InvitationEvent, InvitationToken, InvitationsCore};
use mailing::FakeMailbox;
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
}

pub fn wired() -> Wired {
    wired_guarded_by(Arc::new(AllowingEverything))
}

pub fn wired_guarded_by(authorizer: Arc<dyn Authorizer>) -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let store = Arc::new(InMemoryEventStore::new());
    let mailbox = Arc::new(FakeMailbox::default());
    let catalog = Arc::new(InMemoryInvitationCatalog::new());
    let published = Arc::new(FakePublisher::default());
    let outbox = Arc::new(InMemoryOutbox::new(published.clone(), clock.clone()));

    Wired {
        invitations: InvitationsCore::new(
            store.clone(),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryThrottles::default()),
            catalog.clone(),
            mailbox.clone(),
            authorizer,
            clock.clone(),
        ),
        store,
        catalog,
        mailbox,
        clock,
        outbox,
        published,
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

    pub fn links_mailed_to(&self, address: &str) -> Vec<String> {
        self.mailbox
            .sent_to(address)
            .iter()
            .filter_map(|mail| {
                mail.body
                    .split_whitespace()
                    .find(|word| word.contains("/invitation#"))
                    .map(str::to_owned)
            })
            .collect()
    }

    pub fn token_mailed_to(&self, address: &str) -> InvitationToken {
        let link = self
            .links_mailed_to(address)
            .pop()
            .expect("a link should have been mailed");

        InvitationToken::presented(
            link.split_once("/invitation#")
                .expect("the link carries its token")
                .1,
        )
    }
}
