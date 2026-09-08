use std::env;

use sqlx::AssertSqlSafe;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use time::OffsetDateTime;
use uuid::Uuid;

const DATABASE_URL: &str = "DATABASE_URL";
const DEFAULT_URL: &str = "postgres://weaveling:weaveling@127.0.0.1:5432/weaveling";
const PREFIX: &str = "fixture";
const STALE_AFTER: i64 = 3_600;
const HEX_LENGTH: usize = 8;

pub struct PostgresFixture {
    stem: String,
}

pub fn database_url() -> String {
    env::var(DATABASE_URL).unwrap_or_else(|_| DEFAULT_URL.to_owned())
}

impl PostgresFixture {
    pub async fn setup() -> Self {
        let base = connect(1).await;

        drop_stale(&base, OffsetDateTime::now_utc()).await;
        base.close().await;

        Self {
            stem: stem(OffsetDateTime::now_utc()),
        }
    }

    pub async fn create_schema(&self, feature: &str) -> PgPool {
        let schema = self.schema_of(feature);
        let base = connect(1).await;

        execute(&base, format!(r#"CREATE SCHEMA "{schema}""#))
            .await
            .expect("a fixture schema should be creatable");
        base.close().await;

        connect_to(&schema).await
    }

    pub fn schema_of(&self, feature: &str) -> String {
        format!("{}_{}", self.stem, sanitized(feature))
    }

    pub async fn cleanup(self) {
        let base = connect(1).await;

        for schema in schemas_like(&base, &format!(r"{}\_%", self.stem)).await {
            let _ = execute(
                &base,
                format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#),
            )
            .await;
        }

        base.close().await;
    }
}

async fn execute(pool: &PgPool, statement: String) -> Result<(), sqlx::Error> {
    let mut connection = pool.acquire().await?;
    sqlx::raw_sql(AssertSqlSafe(statement))
        .execute(&mut *connection)
        .await?;

    Ok(())
}

async fn connect(connections: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(connections)
        .connect(&database_url())
        .await
        .expect("postgres should be reachable — try `docker compose up -d`")
}

async fn connect_to(schema: &str) -> PgPool {
    let search_path = format!(r#"SET search_path TO "{schema}""#);

    PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _| {
            let search_path = search_path.clone();

            Box::pin(async move {
                sqlx::raw_sql(AssertSqlSafe(search_path))
                    .execute(&mut *connection)
                    .await?;

                Ok(())
            })
        })
        .connect(&database_url())
        .await
        .expect("postgres should be reachable — try `docker compose up -d`")
}

fn stem(now: OffsetDateTime) -> String {
    let hex = Uuid::new_v4().simple().to_string();

    format!("{PREFIX}_{}_{}", now.unix_timestamp(), &hex[..HEX_LENGTH])
}

fn sanitized(feature: &str) -> String {
    feature
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_stale(schema: &str, now: OffsetDateTime) -> bool {
    let Some(stamp) = schema.strip_prefix(&format!("{PREFIX}_")) else {
        return false;
    };
    let Some((seconds, rest)) = stamp.split_once('_') else {
        return false;
    };
    let hex = rest.split_once('_').map_or(rest, |(hex, _)| hex);

    if hex.len() != HEX_LENGTH || !hex.chars().all(|letter| letter.is_ascii_hexdigit()) {
        return false;
    }

    seconds
        .parse::<i64>()
        .is_ok_and(|then| now.unix_timestamp() - then > STALE_AFTER)
}

async fn schemas_like(base: &PgPool, pattern: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>("SELECT nspname::text FROM pg_namespace WHERE nspname LIKE $1")
        .bind(pattern)
        .fetch_all(base)
        .await
        .unwrap_or_default()
}

async fn drop_stale(base: &PgPool, now: OffsetDateTime) {
    let known = schemas_like(base, &format!(r"{PREFIX}\_%")).await;

    for schema in known.iter().filter(|schema| is_stale(schema, now)) {
        let _ = execute(base, format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn a_schema(seconds: i64, feature: &str) -> String {
        format!("{}_{}", stem(at(seconds)), sanitized(feature))
    }

    #[test]
    fn a_fixture_schema_says_when_it_was_made_and_who_owns_it() {
        let named = a_schema(1_000, "outline");

        assert!(named.starts_with("fixture_1000_"));
        assert!(named.ends_with("_outline"));
    }

    #[test]
    fn two_features_of_one_fixture_never_share_a_schema() {
        let stem = stem(at(1_000));

        assert_ne!(
            format!("{stem}_{}", sanitized("outline")),
            format!("{stem}_{}", sanitized("pieces"))
        );
    }

    #[test]
    fn two_fixtures_made_together_are_still_distinct() {
        assert_ne!(stem(at(1_000)), stem(at(1_000)));
    }

    #[test]
    fn a_feature_name_cannot_carry_anything_but_letters_into_a_statement() {
        assert_eq!(
            sanitized(r#"outline"; drop table events; --"#),
            "outlinedroptableevents"
        );
        assert_eq!(sanitized("Event-Sourcing"), "eventsourcing");
    }

    #[test]
    fn a_schema_from_an_hour_ago_was_is_stale() {
        assert!(is_stale(
            &a_schema(1_000, "outline"),
            at(1_000 + STALE_AFTER + 1)
        ));
        assert!(is_stale(&stem(at(1_000)), at(1_000 + STALE_AFTER + 1)));
    }

    #[test]
    fn a_schema_from_a_moment_ago_is_someone_elses_test() {
        assert!(!is_stale(
            &a_schema(1_000, "outline"),
            at(1_000 + STALE_AFTER)
        ));
    }

    #[test]
    fn a_schema_that_is_not_ours_is_left_alone() {
        assert!(!is_stale("public", at(9_999)));
        assert!(!is_stale("fixture_nonsense", at(9_999)));
        assert!(!is_stale("fixture_", at(9_999)));
        assert!(!is_stale(
            r#"fixture_1000_"; drop table events; --"#,
            at(9_999)
        ));
    }
}
