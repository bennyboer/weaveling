use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, Executor, PgPool};

use crate::Unprepared;

const POOLED: u32 = 5;
const DUPLICATE_DATABASE: &str = "42P04";

#[async_trait]
pub trait Databases: Send + Sync {
    async fn ready(&self, feature: &str) -> Result<PgPool, Unprepared>;
}

pub struct ServerDatabases {
    server: String,
}

impl ServerDatabases {
    pub fn on(server: &str) -> Self {
        Self {
            server: server.to_owned(),
        }
    }
}

#[async_trait]
impl Databases for ServerDatabases {
    async fn ready(&self, feature: &str) -> Result<PgPool, Unprepared> {
        ensure(&self.server, feature).await?;

        connect(&self.server, feature).await
    }
}

pub fn named(feature: &str) -> String {
    format!("weaveling_{feature}")
}

pub fn beside(server: &str, database: &str) -> String {
    let (address, query) = match server.split_once('?') {
        Some((address, query)) => (address, Some(query)),
        None => (server, None),
    };
    let root = address.rsplit_once('/').map_or(address, |(root, _)| root);

    match query {
        Some(query) => format!("{root}/{database}?{query}"),
        None => format!("{root}/{database}"),
    }
}

pub async fn ensure(server: &str, feature: &str) -> Result<(), Unprepared> {
    let database = named(feature);
    let keeper = joined(server, &database).await?;

    let known: Option<i32> = sqlx::query_scalar("SELECT 1 FROM pg_database WHERE datname = $1")
        .bind(&database)
        .fetch_optional(&keeper)
        .await
        .map_err(|why| Unprepared::Unreachable {
            database: database.clone(),
            why: why.to_string(),
        })?;

    if known.is_none() {
        let mut held = keeper
            .acquire()
            .await
            .map_err(|why| Unprepared::Unreachable {
                database: database.clone(),
                why: why.to_string(),
            })?;

        match held
            .execute(AssertSqlSafe(format!(r#"CREATE DATABASE "{database}""#)))
            .await
        {
            Ok(_) => tracing::info!(database, "created a database for a feature that had none"),
            Err(why) if made_meanwhile(&why) => {}
            Err(why) => {
                return Err(Unprepared::Uncreatable {
                    database,
                    why: why.to_string(),
                });
            }
        }
    }

    keeper.close().await;

    Ok(())
}

pub async fn connect(server: &str, feature: &str) -> Result<PgPool, Unprepared> {
    let database = named(feature);

    PgPoolOptions::new()
        .max_connections(POOLED)
        .after_connect(|connection, _| {
            Box::pin(async move {
                connection
                    .execute(transacting::postgres::SERIALIZABLE)
                    .await?;

                Ok(())
            })
        })
        .connect(&beside(server, &database))
        .await
        .map_err(|why| Unprepared::Unreachable {
            database,
            why: why.to_string(),
        })
}

pub async fn lay_out(feature: &str, pool: &PgPool, migrations: Migrator) -> Result<(), Unprepared> {
    migrations
        .run(pool)
        .await
        .map_err(|why| Unprepared::Unmigrated {
            database: named(feature),
            why: why.to_string(),
        })
}

fn made_meanwhile(why: &sqlx::Error) -> bool {
    matches!(why, sqlx::Error::Database(reason)
        if reason.is_unique_violation() || reason.code().as_deref() == Some(DUPLICATE_DATABASE))
}

async fn joined(server: &str, about: &str) -> Result<PgPool, Unprepared> {
    PgPoolOptions::new()
        .max_connections(1)
        .connect(server)
        .await
        .map_err(|why| Unprepared::Unreachable {
            database: about.to_owned(),
            why: why.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use tokio::task::JoinSet;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn a_feature_database_sits_beside_the_one_we_were_pointed_at() {
        assert_eq!(
            beside(
                "postgres://weaveling:weaveling@127.0.0.1:5432/weaveling",
                "weaveling_outline"
            ),
            "postgres://weaveling:weaveling@127.0.0.1:5432/weaveling_outline"
        );
    }

    #[test]
    fn the_query_string_survives_being_pointed_elsewhere() {
        assert_eq!(
            beside(
                "postgres://host/weaveling?sslmode=require&application_name=weaveling",
                "weaveling_ideas"
            ),
            "postgres://host/weaveling_ideas?sslmode=require&application_name=weaveling"
        );
    }

    #[test]
    fn every_feature_is_named_the_same_way_the_init_script_names_it() {
        assert_eq!(named("outline"), "weaveling_outline");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn instances_starting_at_once_on_a_fresh_server_all_find_the_database_ready() {
        let server = test_harness::database_url();
        let feature = format!("racing_{}", Uuid::new_v4().simple());

        let mut starting = JoinSet::new();
        for _ in 0..8 {
            let (server, feature) = (server.clone(), feature.clone());
            starting.spawn(async move { ensure(&server, &feature).await });
        }
        let started = starting.join_all().await;

        let keeper = joined(&server, &feature)
            .await
            .expect("the server is reachable");
        keeper
            .execute(AssertSqlSafe(format!(
                r#"DROP DATABASE IF EXISTS "{}""#,
                named(&feature)
            )))
            .await
            .expect("the raced database can be dropped");
        keeper.close().await;

        for outcome in started {
            assert!(
                outcome.is_ok(),
                "one created it, the others find it made: {outcome:?}"
            );
        }
    }
}
