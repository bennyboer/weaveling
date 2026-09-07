use std::env;

use sqlx::AssertSqlSafe;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use time::OffsetDateTime;
use uuid::Uuid;

const ELSEWHERE: &str = "DATABASE_URL";
const NEARBY: &str = "postgres://weaveling:weaveling@127.0.0.1:5432/weaveling";
const PREFIX: &str = "scratch";
const FORGOTTEN: i64 = 3_600;
const SHORT: usize = 8;

pub struct Scratch {
    pool: PgPool,
    schema: String,
}

pub fn address() -> String {
    env::var(ELSEWHERE).unwrap_or_else(|_| NEARBY.to_owned())
}

impl Scratch {
    pub async fn fresh() -> Self {
        let schema = named(OffsetDateTime::now_utc());
        let keeper = joined(1).await;

        sweep(&keeper, OffsetDateTime::now_utc()).await;
        run(&keeper, format!(r#"create schema "{schema}""#))
            .await
            .expect("a scratch schema should be creatable");
        keeper.close().await;

        let pool = looking_at(&schema).await;
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("the migrations should apply to an empty schema");

        Self { pool, schema }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn schema(&self) -> &str {
        &self.schema
    }

    pub async fn discard(self) {
        self.pool.close().await;

        let keeper = joined(1).await;
        let _ = run(
            &keeper,
            format!(r#"drop schema if exists "{}" cascade"#, self.schema),
        )
        .await;
        keeper.close().await;
    }
}

async fn run(pool: &PgPool, statement: String) -> Result<(), sqlx::Error> {
    let mut held = pool.acquire().await?;
    sqlx::raw_sql(AssertSqlSafe(statement))
        .execute(&mut *held)
        .await?;

    Ok(())
}

async fn joined(most: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(most)
        .connect(&address())
        .await
        .expect("postgres should be reachable — try `docker compose up -d`")
}

async fn looking_at(schema: &str) -> PgPool {
    let inside = format!(r#"set search_path to "{schema}""#);

    PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _| {
            let inside = inside.clone();

            Box::pin(async move {
                sqlx::raw_sql(AssertSqlSafe(inside))
                    .execute(&mut *connection)
                    .await?;

                Ok(())
            })
        })
        .connect(&address())
        .await
        .expect("postgres should be reachable — try `docker compose up -d`")
}

fn named(now: OffsetDateTime) -> String {
    let short = Uuid::new_v4().simple().to_string();

    format!("{PREFIX}_{}_{}", now.unix_timestamp(), &short[..SHORT])
}

fn left_behind(schema: &str, now: OffsetDateTime) -> bool {
    let Some(stamp) = schema.strip_prefix(&format!("{PREFIX}_")) else {
        return false;
    };
    let Some((seconds, tail)) = stamp.split_once('_') else {
        return false;
    };

    if tail.len() != SHORT || !tail.chars().all(|letter| letter.is_ascii_hexdigit()) {
        return false;
    }

    seconds
        .parse::<i64>()
        .is_ok_and(|then| now.unix_timestamp() - then > FORGOTTEN)
}

async fn sweep(keeper: &PgPool, now: OffsetDateTime) {
    let Ok(known) = sqlx::query_scalar::<_, String>(
        "select nspname::text from pg_namespace where nspname like $1",
    )
    .bind(format!("{PREFIX}\\_%"))
    .fetch_all(keeper)
    .await
    else {
        return;
    };

    for schema in known.iter().filter(|schema| left_behind(schema, now)) {
        let _ = run(
            keeper,
            format!(r#"drop schema if exists "{schema}" cascade"#),
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    #[test]
    fn a_scratch_schema_carries_the_moment_it_was_made() {
        let named = named(at(1_000));

        assert!(named.starts_with("scratch_1000_"));
        assert_eq!(named.len(), "scratch_1000_".len() + SHORT);
    }

    #[test]
    fn two_scratch_schemas_made_together_are_still_distinct() {
        assert_ne!(named(at(1_000)), named(at(1_000)));
    }

    #[test]
    fn a_schema_from_an_hour_ago_was_left_behind() {
        assert!(left_behind(&named(at(1_000)), at(1_000 + FORGOTTEN + 1)));
    }

    #[test]
    fn a_schema_from_a_moment_ago_is_someone_elses_test() {
        assert!(!left_behind(&named(at(1_000)), at(1_000 + FORGOTTEN)));
    }

    #[test]
    fn a_schema_that_is_not_ours_is_left_alone() {
        assert!(!left_behind("public", at(9_999)));
        assert!(!left_behind("scratch_nonsense", at(9_999)));
        assert!(!left_behind("scratch_", at(9_999)));
        assert!(!left_behind(
            r#"scratch_1000_"; drop table events; --"#,
            at(9_999)
        ));
    }
}
