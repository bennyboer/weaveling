use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use access::{AccessCatalog, AccessError, HearsAccess, InMemoryAccess, KeptAccess, Role};
use acting::Acting;
use async_trait::async_trait;
use authorizing::{AllowingEverything, Authorizer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore, Version};
use inbox::{EffectHandler, InMemoryInbox, Inboxed, InboxedEffect, KeptHandler};
use invitations_catalog::{InMemoryInvitationCatalog, InMemoryTokenKeys};
use invitations_contract::ISSUED;
use invitations_core::{InvitationEvent, InvitationToken, InvitationsCore};
use invitations_messaging::MailIssuedInvitationListener;
use mailing::{FakeMailbox, Mail};
use messaging::{FakePublisher, Listener, Message};
use outbox::{InMemoryOutbox, Outbox};
use throttling::InMemoryThrottles;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};

pub const ORIGIN: &str = "https://weaveling.example.com";

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
    pub mailing: InboxedEffect<MailIssuedInvitationListener>,
    pub access: Arc<Unreliable>,
    handed: AtomicUsize,
    versions: AtomicU64,
}

/// Who may do what, kept in memory, which can be made unreachable.
#[derive(Default)]
pub struct Unreliable {
    kept: InMemoryAccess,
    down: AtomicBool,
}

impl Unreliable {
    pub fn go_down(&self) {
        self.down.store(true, Ordering::SeqCst);
    }

    fn reachable(&self) -> Result<(), AccessError> {
        if self.down.load(Ordering::SeqCst) {
            Err(AccessError::Backend("the list is down".into()))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl AccessCatalog for Unreliable {
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_role(transaction, project, account, version, role, at)
            .await
    }

    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_disbanded(transaction, project, version, at)
            .await
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_trash(transaction, project, version, in_the_trash)
            .await
    }

    async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_operator(transaction, account, version, operator, at)
            .await
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError> {
        self.kept
            .forget_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError> {
        self.reachable()?;
        self.kept.role_of(project, account).await
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.is_in_the_trash(project).await
    }

    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError> {
        self.reachable()?;
        self.kept.projects_of(account).await
    }

    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.share_a_project(one, other).await
    }

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.operates(account).await
    }
}

pub fn a_workbench() -> Wired {
    a_workbench_with(false)
}

/// A workbench with accounts: who may do what is what the feature heard of the members, or was given by `give`.
pub fn a_workbench_with_accounts() -> Wired {
    a_workbench_with(true)
}

fn a_workbench_with(with_accounts: bool) -> Wired {
    let clock = Arc::new(FixedClock::new(clock::at(1_700_000_000)));
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
    let access = Arc::new(Unreliable::default());
    let kept = KeptAccess::new(access.clone());
    let authorizer: Arc<dyn Authorizer> = if with_accounts {
        Arc::new(kept.clone())
    } else {
        Arc::new(AllowingEverything)
    };

    let invitations = InvitationsCore::new(
        store.clone(),
        Arc::new(InMemoryTransactions::new()),
        keys.clone(),
        Arc::new(InMemoryThrottles::default()),
        catalog.clone(),
        mailbox.clone(),
        kept,
        authorizer,
        clock.clone(),
    );

    Wired {
        mailing: inboxed_effect(MailIssuedInvitationListener::new(invitations.clone())),
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
        access,
        handed: AtomicUsize::new(0),
        versions: AtomicU64::new(0),
    }
}

pub fn inboxed<H: KeptHandler>(handler: H) -> Inboxed<H> {
    inboxed_in(handler, InMemoryTransactions::new())
}

pub fn inboxed_in<H: KeptHandler>(handler: H, hearings: InMemoryTransactions) -> Inboxed<H> {
    Inboxed::new(
        handler,
        Arc::new(hearings),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
            1_700_000_000,
        ))))),
    )
}

pub fn inboxed_effect<H: EffectHandler>(handler: H) -> InboxedEffect<H> {
    InboxedEffect::new(
        handler,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
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
    /// Gives an account a role in a project, or takes it away with none, as the members would tell it.
    pub async fn give(&self, project: &str, account: &str, role: Option<Role>) {
        let version = self.versions.fetch_add(1, Ordering::SeqCst) + 1;
        self.invitations
            .remember_role(
                None,
                project,
                &AccountId::from(account),
                Version::of(version),
                role,
            )
            .await
            .expect("remembering a role should succeed");
    }

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
