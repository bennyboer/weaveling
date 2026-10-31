use eventsourcing::{AccountId, Codec};
use notifications_core::{About, NoticeEvent, ProjectLink, Role};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum StoredNoticeEvent {
    Noted {
        account: String,
        about: StoredAbout,
    },
    Dismissed,
    Snapshotted {
        account: String,
        about: StoredAbout,
        #[serde(with = "time::serde::rfc3339")]
        noted_at: OffsetDateTime,
        #[serde(default, with = "time::serde::rfc3339::option")]
        dismissed_at: Option<OffsetDateTime>,
    },
}

#[derive(Serialize, Deserialize)]
enum StoredAbout {
    RoleChanged {
        project: String,
        role: StoredRole,
        by: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
enum StoredRole {
    Owner,
    Author,
    Reader,
}

pub fn codec() -> Codec<NoticeEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredNoticeEvent::from(event))
                .expect("a stored notice event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredNoticeEvent>(body)
                .ok()
                .map(NoticeEvent::from)
        },
    }
}

impl From<&NoticeEvent> for StoredNoticeEvent {
    fn from(event: &NoticeEvent) -> Self {
        match event {
            NoticeEvent::Noted { account, about } => Self::Noted {
                account: account.to_string(),
                about: StoredAbout::from(about),
            },
            NoticeEvent::Dismissed => Self::Dismissed,
            NoticeEvent::Snapshotted {
                account,
                about,
                noted_at,
                dismissed_at,
            } => Self::Snapshotted {
                account: account.to_string(),
                about: StoredAbout::from(about),
                noted_at: *noted_at,
                dismissed_at: *dismissed_at,
            },
        }
    }
}

impl From<StoredNoticeEvent> for NoticeEvent {
    fn from(stored: StoredNoticeEvent) -> Self {
        match stored {
            StoredNoticeEvent::Noted { account, about } => Self::Noted {
                account: AccountId::from(account),
                about: About::from(about),
            },
            StoredNoticeEvent::Dismissed => Self::Dismissed,
            StoredNoticeEvent::Snapshotted {
                account,
                about,
                noted_at,
                dismissed_at,
            } => Self::Snapshotted {
                account: AccountId::from(account),
                about: About::from(about),
                noted_at,
                dismissed_at,
            },
        }
    }
}

impl From<&About> for StoredAbout {
    fn from(about: &About) -> Self {
        match about {
            About::RoleChanged { project, role, by } => Self::RoleChanged {
                project: project.to_string(),
                role: StoredRole::from(*role),
                by: by.as_ref().map(ToString::to_string),
            },
        }
    }
}

impl From<StoredAbout> for About {
    fn from(stored: StoredAbout) -> Self {
        match stored {
            StoredAbout::RoleChanged { project, role, by } => Self::RoleChanged {
                project: ProjectLink::from(project.as_str()),
                role: Role::from(role),
                by: by.map(AccountId::from),
            },
        }
    }
}

impl From<Role> for StoredRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Owner => Self::Owner,
            Role::Author => Self::Author,
            Role::Reader => Self::Reader,
        }
    }
}

impl From<StoredRole> for Role {
    fn from(stored: StoredRole) -> Self {
        match stored {
            StoredRole::Owner => Self::Owner,
            StoredRole::Author => Self::Author,
            StoredRole::Reader => Self::Reader,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(event: NoticeEvent) -> Option<NoticeEvent> {
        let codec = codec();

        (codec.event)((codec.body)(&event))
    }

    fn a_role_changed(role: Role, by: Option<&str>) -> About {
        About::RoleChanged {
            project: ProjectLink::from("project_1"),
            role,
            by: by.map(AccountId::from),
        }
    }

    #[test]
    fn every_notice_event_reads_back_as_it_was_written() {
        for event in [
            NoticeEvent::Noted {
                account: AccountId::from("account_bo"),
                about: a_role_changed(Role::Owner, Some("account_ada")),
            },
            NoticeEvent::Noted {
                account: AccountId::from("account_bo"),
                about: a_role_changed(Role::Author, None),
            },
            NoticeEvent::Dismissed,
            NoticeEvent::Snapshotted {
                account: AccountId::from("account_bo"),
                about: a_role_changed(Role::Reader, Some("account_ada")),
                noted_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
                    .expect("a plausible moment"),
                dismissed_at: Some(
                    OffsetDateTime::from_unix_timestamp(1_700_000_600).expect("a plausible moment"),
                ),
            },
        ] {
            assert_eq!(round_trip(event.clone()), Some(event));
        }
    }

    #[test]
    fn what_is_no_notice_event_reads_as_nothing() {
        assert_eq!(
            (codec().event)(serde_json::json!({ "Forgotten": {} })),
            None
        );
    }
}
