use std::sync::Arc;

use accounts_core::{AccountEvent, AccountService};
use accounts_secrets::InMemorySecrets;
use axum::Router;
use clock::FixedClock;
use eventsourcing::{InMemoryEventStore, Recorded};
use mailing::FakeMailbox;
use messaging::{FakePublisher, Message, RoutingKey};
use outbox::InMemoryOutbox;
use serde_json::json;
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

fn each_event_told(happened: &Recorded<AccountEvent>) -> Option<Message> {
    Some(Message::opening(
        RoutingKey::parse("account.happened").expect("a plain routing key"),
        json!(happened.metadata.aggregate.as_str()),
        happened.metadata.occurred_at,
    ))
}

pub fn wired() -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let told = Arc::new(InMemoryOutbox::new(
        Arc::new(FakePublisher::default()),
        clock.clone(),
    ));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        told.clone(),
        each_event_told,
    ));
    let secrets = Arc::new(InMemorySecrets::default());
    let mailbox = Arc::new(FakeMailbox::default());
    let accounts = AccountService::new(
        store.clone(),
        secrets.clone(),
        secrets.clone(),
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
