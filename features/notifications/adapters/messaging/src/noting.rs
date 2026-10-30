use async_trait::async_trait;
use eventsourcing::AccountId;
use members_contract::{MembersEventDTO, ROLE_CHANGED, role};
use messaging::{Failure, Handler, Message};
use notifications_core::{About, NotificationsCore, ProjectLink, Role};

const AN_ACCOUNT: &str = "account";

pub struct NoteRoleChangedListener {
    notifications: NotificationsCore,
}

impl NoteRoleChangedListener {
    pub fn new(notifications: NotificationsCore) -> Self {
        Self { notifications }
    }
}

#[async_trait]
impl Handler for NoteRoleChangedListener {
    const NAME: &'static str = "note-role-changed";
    const LISTENS_TO: &'static [&'static str] = &[ROLE_CHANGED];
    const WHEN_REFUSED: &'static str =
        "Someone whose role in a project was changed was not told so in their notifications.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let changed = eventpublishing::published_in::<MembersEventDTO>(message)?;
        let MembersEventDTO::RoleChanged { account, to } = changed.event.body else {
            return Err("a changed role tells no account and no role".into());
        };
        let role = to_role(&to).ok_or_else(|| format!("{to:?} is no role"))?;
        let by = (changed.agent.kind == AN_ACCOUNT)
            .then_some(changed.agent.id)
            .flatten()
            .map(AccountId::from);
        let cause = format!(
            "{}:{}@{}",
            changed.aggregate.kind, changed.aggregate.id, changed.aggregate.version
        );

        self.notifications
            .note(
                &cause,
                AccountId::from(account),
                About::RoleChanged {
                    project: ProjectLink::from(changed.aggregate.id.as_str()),
                    role,
                    by,
                },
            )
            .await?;

        Ok(())
    }
}

fn to_role(named: &str) -> Option<Role> {
    match named {
        role::OWNER => Some(Role::Owner),
        role::AUTHOR => Some(Role::Author),
        role::READER => Some(Role::Reader),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use clock::FixedClock;
    use eventsourcing::{Agent, InMemoryEventStore};
    use messaging::{Listener, RoutingKey};
    use notifications_core::KIND;
    use registry::{InMemoryRegistry, Registry};
    use serde_json::{Value, json};
    use time::OffsetDateTime;

    use super::*;

    struct Bench {
        registry: Arc<InMemoryRegistry>,
        notifications: NotificationsCore,
        listener: NoteRoleChangedListener,
    }

    fn bench() -> Bench {
        let registry = Arc::new(InMemoryRegistry::new());
        let notifications = NotificationsCore::new(
            Arc::new(InMemoryEventStore::new()),
            registry.clone(),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        Bench {
            listener: NoteRoleChangedListener::new(notifications.clone()),
            notifications,
            registry,
        }
    }

    fn changed(body: Value, agent: Value) -> Message {
        Message::opening(
            RoutingKey::parse(ROLE_CHANGED).expect("a declared key"),
            json!({
                "event": body,
                "aggregate": { "id": "project_1", "kind": "members", "version": 4 },
                "agent": agent,
                "occurred_at": "1970-01-01T00:00:00Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    fn to(role: &str) -> Value {
        json!({ "version": 0, "name": "ROLE_CHANGED", "account": "account_bo", "to": role })
    }

    fn ada() -> Value {
        json!({ "kind": "account", "id": "account_ada" })
    }

    async fn noted(bench: &Bench) -> Option<About> {
        let id = bench
            .registry
            .holder(KIND.as_str(), "members:project_1@4")
            .await
            .expect("asking should succeed")?;
        let standing = bench
            .notifications
            .read(&id, &Agent::Account(AccountId::from("account_bo")))
            .await
            .expect("the notice is bo's to read");

        Some(standing.state.about().clone())
    }

    #[tokio::test]
    async fn whoever_s_role_changed_is_noted_it_with_who_changed_it_however_often_it_is_told() {
        for (named, role) in [
            ("owner", Role::Owner),
            ("author", Role::Author),
            ("reader", Role::Reader),
        ] {
            let bench = bench();

            for _ in 0..2 {
                bench
                    .listener
                    .take_in(&changed(to(named), ada()))
                    .await
                    .expect("messages come at least once");
            }

            assert_eq!(
                noted(&bench).await,
                Some(About::RoleChanged {
                    project: ProjectLink::from("project_1"),
                    role,
                    by: Some(AccountId::from("account_ada")),
                })
            );
        }
    }

    #[tokio::test]
    async fn a_role_changed_by_the_system_names_nobody_who_changed_it() {
        let bench = bench();

        bench
            .listener
            .take_in(&changed(
                to("author"),
                json!({ "kind": "system", "id": "account_ada" }),
            ))
            .await
            .expect("noting should succeed");

        assert_eq!(
            noted(&bench).await,
            Some(About::RoleChanged {
                project: ProjectLink::from("project_1"),
                role: Role::Author,
                by: None,
            })
        );
    }

    #[tokio::test]
    async fn a_change_the_listener_cannot_read_belongs_in_dead_letters() {
        let bench = bench();

        for unreadable in [
            changed(to("editor"), ada()),
            changed(
                json!({ "version": 0, "name": "LEFT", "account": "account_bo" }),
                ada(),
            ),
            Message::opening(
                RoutingKey::parse(ROLE_CHANGED).expect("a declared key"),
                json!({ "something": "else" }),
                OffsetDateTime::UNIX_EPOCH,
            ),
        ] {
            assert!(bench.listener.take_in(&unreadable).await.is_err());
        }
        assert_eq!(noted(&bench).await, None);
    }

    #[tokio::test]
    async fn only_a_changed_role_is_heard() {
        let listener = bench().listener;

        assert!(listener.hears(&RoutingKey::parse(ROLE_CHANGED).expect("a declared key")));
        assert!(
            !listener.hears(&RoutingKey::parse(members_contract::JOINED).expect("a declared key"))
        );
    }
}
