use std::sync::Arc;

use acting::Acting;
use clock::FixedClock;
use eventsourcing::{Agent, InMemoryEventStore};
use invitations_catalog::InMemoryInvitationCatalog;
use invitations_core::{InvitationEvent, InvitationService, InvitationToken};
use mailing::FakeMailbox;
use throttling::InMemoryThrottles;
use time::{Duration, OffsetDateTime};

pub const ORIGIN: &str = "https://weaveling.example.com";

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

pub struct Wired {
    pub invitations: InvitationService,
    pub store: Arc<InMemoryEventStore<InvitationEvent>>,
    pub catalog: Arc<InMemoryInvitationCatalog>,
    pub mailbox: Arc<FakeMailbox>,
    pub clock: Arc<FixedClock>,
}

pub fn wired() -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let store = Arc::new(InMemoryEventStore::new());
    let mailbox = Arc::new(FakeMailbox::default());
    let catalog = Arc::new(InMemoryInvitationCatalog::new());

    Wired {
        invitations: InvitationService::new(
            store.clone(),
            Arc::new(InMemoryThrottles::default()),
            catalog.clone(),
            mailbox.clone(),
            clock.clone(),
        ),
        store,
        catalog,
        mailbox,
        clock,
    }
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
    pub fn links_mailed_to(&self, address: &str) -> Vec<String> {
        self.mailbox
            .sent_to(address)
            .iter()
            .filter_map(|mail| {
                mail.body
                    .split_whitespace()
                    .find(|word| word.contains("?invitation="))
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
            link.split_once("?invitation=")
                .expect("the link carries its token")
                .1,
        )
    }
}
