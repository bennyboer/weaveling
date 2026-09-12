use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, Executor, PgPool};
use thiserror::Error;

const POOLED: u32 = 5;

#[derive(Debug, Error)]
pub enum Unprepared {
    #[error("{database} could not be reached: {why}")]
    Unreachable { database: String, why: String },
    #[error("{database} could not be created: {why}")]
    Uncreatable { database: String, why: String },
    #[error("the schema of {database} could not be laid down: {why}")]
    Unmigrated { database: String, why: String },
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

        held.execute(AssertSqlSafe(format!(r#"CREATE DATABASE "{database}""#)))
            .await
            .map_err(|why| Unprepared::Uncreatable {
                database: database.clone(),
                why: why.to_string(),
            })?;

        tracing::info!(database, "created a database for a feature that had none");
    }

    keeper.close().await;

    Ok(())
}

pub async fn connect(server: &str, feature: &str) -> Result<PgPool, Unprepared> {
    let database = named(feature);

    PgPoolOptions::new()
        .max_connections(POOLED)
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
                "weaveling_pieces"
            ),
            "postgres://host/weaveling_pieces?sslmode=require&application_name=weaveling"
        );
    }

    #[test]
    fn every_feature_is_named_the_same_way_the_init_script_names_it() {
        assert_eq!(named("outline"), "weaveling_outline");
    }
}
