use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::time::Duration;

use outbox::Cadence;
use thiserror::Error;
use wiring::Storage;

use crate::settings::Setting;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
use std::sync::Arc;

#[cfg(feature = "postgres")]
use wiring::ServerDatabases;

#[cfg(feature = "sqlite")]
use wiring::sqlite::DataDirectory;

pub const WEAVELING_DATABASE_URL: &str = "WEAVELING_DATABASE_URL";
pub const WEAVELING_DATA: &str = "WEAVELING_DATA";

const LOCAL_POLLING: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    InMemory,
    Postgres { server: String },
    Sqlite { directory: PathBuf },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Unchosen {
    #[error("both {one} and {other} name somewhere to keep the work; set only one")]
    Ambiguous { one: String, other: String },
    #[error("{backend} was asked for, but this build left it out")]
    NotBuiltIn { backend: &'static str },
}

impl Backend {
    pub fn chosen(database_url: Option<Setting>, data: Option<Setting>) -> Result<Self, Unchosen> {
        match (given(database_url), given(data)) {
            (Some(server), Some(directory)) => Err(Unchosen::Ambiguous {
                one: server.source,
                other: directory.source,
            }),
            (Some(server), None) => Ok(Self::Postgres {
                server: server.value,
            }),
            (None, Some(directory)) => Ok(Self::Sqlite {
                directory: directory.value.into(),
            }),
            (None, None) => Ok(Self::InMemory),
        }
    }

    pub fn storage(&self) -> Result<Storage, Unchosen> {
        match self {
            Self::InMemory => Ok(Storage::InMemory),
            Self::Postgres { server } => on_postgres(server),
            Self::Sqlite { directory } => on_sqlite(directory),
        }
    }

    pub fn cadence(&self) -> Cadence {
        match self {
            Self::Sqlite { .. } => Cadence {
                deliver_every: LOCAL_POLLING,
                ..Cadence::default()
            },
            Self::InMemory | Self::Postgres { .. } => Cadence::default(),
        }
    }
}

impl Display for Backend {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InMemory => write!(f, "in memory, gone when the process stops"),
            Self::Postgres { .. } => write!(f, "on PostgreSQL"),
            Self::Sqlite { directory } => {
                write!(f, "in SQLite files under {}", directory.display())
            }
        }
    }
}

fn given(setting: Option<Setting>) -> Option<Setting> {
    setting.filter(|setting| !setting.value.trim().is_empty())
}

#[cfg(feature = "postgres")]
fn on_postgres(server: &str) -> Result<Storage, Unchosen> {
    Ok(Storage::Postgres(Arc::new(ServerDatabases::on(server))))
}

#[cfg(not(feature = "postgres"))]
fn on_postgres(_server: &str) -> Result<Storage, Unchosen> {
    Err(Unchosen::NotBuiltIn {
        backend: "PostgreSQL",
    })
}

#[cfg(feature = "sqlite")]
fn on_sqlite(directory: &Path) -> Result<Storage, Unchosen> {
    Ok(Storage::Sqlite(Arc::new(DataDirectory::at(directory))))
}

#[cfg(not(feature = "sqlite"))]
fn on_sqlite(_directory: &Path) -> Result<Storage, Unchosen> {
    Err(Unchosen::NotBuiltIn { backend: "SQLite" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(value: &str) -> Option<Setting> {
        Some(Setting::new("a setting", value))
    }

    #[test]
    fn nothing_configured_keeps_the_work_in_memory() {
        // expect: nothing configured keeps the work in memory
        assert_eq!(Backend::chosen(None, None), Ok(Backend::InMemory));
    }

    #[test]
    fn a_database_url_means_postgres() {
        // expect: a database url chooses postgres
        assert_eq!(
            Backend::chosen(set("postgres://somewhere/weaveling"), None),
            Ok(Backend::Postgres {
                server: "postgres://somewhere/weaveling".to_owned()
            })
        );
    }

    #[test]
    fn a_data_directory_means_local_files() {
        // expect: a data directory chooses local files
        assert_eq!(
            Backend::chosen(None, set("/home/author/weaveling")),
            Ok(Backend::Sqlite {
                directory: PathBuf::from("/home/author/weaveling")
            })
        );
    }

    #[test]
    fn both_at_once_is_refused_rather_than_guessed() {
        // expect: both at once are refused
        assert_eq!(
            Backend::chosen(
                set("postgres://somewhere/weaveling"),
                set("/home/author/weaveling")
            ),
            Err(Unchosen::Ambiguous {
                one: "a setting".to_owned(),
                other: "a setting".to_owned()
            }),
            "picking one silently would put an author's work somewhere they did not expect"
        );
    }

    #[test]
    fn a_blank_setting_counts_as_unset() {
        // expect: blank settings count as unset
        assert_eq!(
            Backend::chosen(set(""), set("  ")),
            Ok(Backend::InMemory),
            "an exported-but-empty variable is how a shell says nothing"
        );
    }

    #[test]
    fn local_files_are_polled_much_more_often_than_the_backstop_elsewhere() {
        // given: the cadence of local files
        let local = Backend::Sqlite {
            directory: PathBuf::from("data"),
        }
        .cadence();

        // then: they are polled much more often than the backstop elsewhere
        assert_eq!(local.deliver_every, LOCAL_POLLING);
        assert!(
            local.deliver_every < Backend::InMemory.cadence().deliver_every,
            "the SQLite outbox is never notified, so its poll is the only thing that delivers"
        );
    }
}
