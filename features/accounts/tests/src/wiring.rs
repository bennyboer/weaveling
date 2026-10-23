use std::sync::Arc;

use accounts_core::{AccountEvent, AccountService, SecretStores};
use accounts_secrets::InMemorySecrets;
use axum::Router;
use clock::FixedClock;
use eventsourcing::InMemoryEventStore;
use mailing::FakeMailbox;
use messaging::FakePublisher;
use outbox::InMemoryOutbox;
use time::{Duration, OffsetDateTime};

pub const ADA: &str = "ada@example.com";
pub const HER_PASSWORD: &str = "correct horse battery";

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

pub struct Wired {
    pub accounts: AccountService,
    pub routes: Router,
    pub store: Arc<InMemoryEventStore<AccountEvent>>,
    pub secrets: Arc<InMemorySecrets>,
    pub told: Arc<InMemoryOutbox>,
    pub mailbox: Arc<FakeMailbox>,
    pub clock: Arc<FixedClock>,
}

pub fn wired() -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let told = Arc::new(InMemoryOutbox::new(
        Arc::new(FakePublisher::default()),
        clock.clone(),
    ));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        told.clone(),
        accounts_messaging::message_for,
    ));
    let secrets = Arc::new(InMemorySecrets::default());
    let mailbox = Arc::new(FakeMailbox::default());
    let accounts = AccountService::new(
        store.clone(),
        SecretStores::kept_by(secrets.clone()),
        mailbox.clone(),
        clock.clone(),
    );

    Wired {
        routes: accounts_rest::router(accounts.clone()),
        accounts,
        store,
        secrets,
        told,
        mailbox,
        clock,
    }
}

impl Wired {
    pub fn codes_sent_to(&self, address: &str) -> Vec<String> {
        self.mailbox
            .sent_to(address)
            .iter()
            .filter_map(|mail| {
                mail.body
                    .split_whitespace()
                    .find(|word| {
                        word.len() == 6 && word.bytes().all(|digit| digit.is_ascii_digit())
                    })
                    .map(str::to_owned)
            })
            .collect()
    }

    pub fn last_code_sent_to(&self, address: &str) -> String {
        self.codes_sent_to(address)
            .pop()
            .expect("a code should have been sent")
    }
}
