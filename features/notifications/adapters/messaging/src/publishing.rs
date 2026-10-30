use eventsourcing::Recorded;
use messaging::Message;
use notifications_contract::{AboutDTO, NoticeEventDTO, role};
use notifications_core::{About, NoticeEvent, Role};

pub fn message_for(happened: &Recorded<NoticeEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &NoticeEvent) -> Option<NoticeEventDTO> {
    Some(match event {
        NoticeEvent::Noted { account, about } => NoticeEventDTO::Noted {
            account: account.to_string(),
            about: about_dto(about),
        },
        NoticeEvent::Dismissed => NoticeEventDTO::Dismissed,
        NoticeEvent::Snapshotted { .. } => return None,
    })
}

pub(crate) fn about_dto(about: &About) -> AboutDTO {
    match about {
        About::RoleChanged { project, role, by } => AboutDTO::RoleChanged {
            project: project.to_string(),
            role: named(*role).to_owned(),
            by: by.as_ref().map(ToString::to_string),
        },
    }
}

pub(crate) fn named(given: Role) -> &'static str {
    match given {
        Role::Owner => role::OWNER,
        Role::Author => role::AUTHOR,
        Role::Reader => role::READER,
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{AccountId, Agent, AggregateContext, AggregateId, EventMetadata, Version};
    use messaging::RoutingKey;
    use notifications_contract::{DISMISSED, NOTED};
    use notifications_core::{KIND, ProjectLink};
    use time::OffsetDateTime;

    use super::*;

    fn recorded(event: NoticeEvent, is_snapshot: bool) -> Recorded<NoticeEvent> {
        Recorded {
            event,
            metadata: EventMetadata {
                aggregate: AggregateId::from("notice_1".to_owned()),
                kind: KIND,
                version: Version::of(1),
                occurred_at: OffsetDateTime::UNIX_EPOCH,
                agent: Agent::System,
                context: AggregateContext::default(),
                is_snapshot,
            },
        }
    }

    #[test]
    fn a_noted_role_change_is_published_with_whom_it_is_for_and_who_changed_it() {
        let noted = recorded(
            NoticeEvent::Noted {
                account: AccountId::from("account_bo"),
                about: About::RoleChanged {
                    project: ProjectLink::from("project_1"),
                    role: Role::Author,
                    by: Some(AccountId::from("account_ada")),
                },
            },
            false,
        );

        let message = message_for(&noted).expect("a noted notice is published");

        assert_eq!(
            message.routing,
            RoutingKey::parse(NOTED).expect("a declared key")
        );
        assert_eq!(
            eventpublishing::published_in::<NoticeEventDTO>(&message)
                .expect("the body reads back")
                .event
                .body,
            NoticeEventDTO::Noted {
                account: "account_bo".to_owned(),
                about: AboutDTO::RoleChanged {
                    project: "project_1".to_owned(),
                    role: "author".to_owned(),
                    by: Some("account_ada".to_owned()),
                },
            }
        );
    }

    #[test]
    fn dismissing_is_published_and_a_snapshot_is_not() {
        let dismissed =
            message_for(&recorded(NoticeEvent::Dismissed, false)).expect("dismissing is published");
        let snapshot = recorded(
            NoticeEvent::Snapshotted {
                account: AccountId::from("account_bo"),
                about: About::RoleChanged {
                    project: ProjectLink::from("project_1"),
                    role: Role::Reader,
                    by: None,
                },
                dismissed: true,
            },
            true,
        );

        assert_eq!(
            dismissed.routing,
            RoutingKey::parse(DISMISSED).expect("a declared key")
        );
        assert!(message_for(&snapshot).is_none());
    }
}
