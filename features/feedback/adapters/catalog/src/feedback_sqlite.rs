use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use eventsourcing::Agent;
use feedback_core::{
    Asked, CatalogError, CataloguedFeedback, Counted, FeedbackCatalog, FeedbackId, Order, Page,
    Pending, Places, ProjectLink, Status, Subject,
};
use sqlx::query::Query;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{AssertSqlSafe, Row, Sqlite, SqlitePool};
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::kept;
use crate::sqlite::unreachable;

const COLUMNS: &str =
    "feedback, project, about, comment, written_by, written_at, submitted_at, edited, done";

const KEEP: &str = "
    INSERT INTO feedback (feedback, project, subject_kind, subject_id, place_kind, place_id, about,
                          comment, written_by, written_at, submitted_at, edited, done)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
    ON CONFLICT (feedback) DO UPDATE
    SET project = excluded.project, subject_kind = excluded.subject_kind,
        subject_id = excluded.subject_id, place_kind = excluded.place_kind,
        place_id = excluded.place_id, about = excluded.about, comment = excluded.comment,
        written_by = excluded.written_by, written_at = excluded.written_at,
        submitted_at = excluded.submitted_at, edited = excluded.edited, done = excluded.done
";

const FORGET: &str = "DELETE FROM feedback WHERE feedback = ?1 RETURNING project, written_by";

/// Which pieces are asked for, as the first four parameters name them, each list as JSON:
/// the project; the subjects as `[kind, id]`, none if not asked by; the places likewise; and the givers.
const ASKED: &str = "
    (?1 IS NULL OR project = ?1)
    AND (?2 IS NULL OR (subject_kind, subject_id) IN (
        SELECT json_extract(value, '$[0]'), json_extract(value, '$[1]') FROM json_each(?2)))
    AND (?3 IS NULL OR (place_kind, place_id) IN (
        SELECT json_extract(value, '$[0]'), json_extract(value, '$[1]') FROM json_each(?3)))
    AND (?4 IS NULL OR written_by IN (SELECT value FROM json_each(?4)))
";

const GIVERS_IN: &str =
    "SELECT giver FROM givers WHERE project = ?1 ORDER BY submitted_at, written_at, feedback";

/// A giver's row made again from their first piece submitted in the project, or gone with the last of them.
const FORGET_GIVER: &str = "DELETE FROM givers WHERE project = ?1 AND giver = ?2";

const FIND_GIVER: &str = "
    INSERT INTO givers (project, giver, submitted_at, written_at, feedback)
    SELECT project, written_by, submitted_at, written_at, feedback
    FROM feedback
    WHERE project = ?1 AND written_by = ?2 AND submitted_at IS NOT NULL
    ORDER BY submitted_at, written_at, feedback
    LIMIT 1
";

const DRAFTS_ON: &str = "SELECT feedback FROM feedback WHERE subject_kind = ?1 AND subject_id = ?2 AND submitted_at IS NULL ORDER BY feedback LIMIT ?3";

const PIECES_ON: &str = "SELECT feedback FROM feedback WHERE subject_kind = ?1 AND subject_id = ?2 ORDER BY feedback LIMIT ?3";

const COUNTED_ON: &str = "
    SELECT subject_kind, subject_id,
           COUNT(*) FILTER (WHERE submitted_at IS NOT NULL AND NOT done) AS not_done,
           COUNT(*) FILTER (WHERE submitted_at IS NULL AND written_by = ?2) AS your_drafts
    FROM feedback
    WHERE (subject_kind, subject_id) IN (
        SELECT json_extract(value, '$[0]'), json_extract(value, '$[1]') FROM json_each(?1))
    GROUP BY subject_kind, subject_id
";

/// Keeps a giver's row as their pieces in the project now stand, in the transaction that changed them.
async fn giver_found_again(
    transaction: &mut Transaction,
    project: &str,
    giver: &str,
) -> Result<(), CatalogError> {
    for refreshing in [FORGET_GIVER, FIND_GIVER] {
        sqlx::query(refreshing)
            .bind(project)
            .bind(giver)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
    }

    Ok(())
}

pub struct SqliteFeedbackCatalog {
    pool: SqlitePool,
}

impl SqliteFeedbackCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn as_json<T: serde::Serialize>(listed: &T) -> String {
    serde_json::to_string(listed).expect("what is asked for is plain data")
}

/// A query with the pieces asked for bound as its first four parameters.
/// The SQL is built only from this module's constants, so it holds nothing anyone asked with.
fn asking(sql: String, asked: &Asked) -> Query<'static, Sqlite, SqliteArguments> {
    let subjects = asked.subjects.as_ref().map(|subjects| {
        as_json(
            &subjects
                .iter()
                .map(kept::subject_columns)
                .map(|(kind, id)| [kind, id])
                .collect::<Vec<[String; 2]>>(),
        )
    });
    let places = asked.places.as_ref().map(|places| match places {
        Places::Text { scenes, sections } => as_json(
            &scenes
                .iter()
                .map(|scene| ["scene".to_owned(), scene.to_string()])
                .chain(
                    sections
                        .iter()
                        .map(|section| ["section".to_owned(), section.to_string()]),
                )
                .collect::<Vec<[String; 2]>>(),
        ),
    });
    let givers = asked.given_by.as_ref().map(|givers| {
        as_json(
            &givers
                .iter()
                .map(kept::agent_to_text)
                .collect::<Vec<String>>(),
        )
    });

    sqlx::query(AssertSqlSafe(sql))
        .bind(asked.project.clone())
        .bind(subjects)
        .bind(places)
        .bind(givers)
}

fn catalogued(row: &SqliteRow) -> Result<CataloguedFeedback, CatalogError> {
    let id: String = row.try_get("feedback").map_err(unreachable)?;
    let project: String = row.try_get("project").map_err(unreachable)?;
    let about: String = row.try_get("about").map_err(unreachable)?;
    let comment: String = row.try_get("comment").map_err(unreachable)?;
    let written_by: String = row.try_get("written_by").map_err(unreachable)?;
    let written_at: i64 = row.try_get("written_at").map_err(unreachable)?;
    let submitted_at: Option<i64> = row.try_get("submitted_at").map_err(unreachable)?;

    Ok(CataloguedFeedback {
        id: FeedbackId::from_str(&id).map_err(unreachable)?,
        project: ProjectLink::from(project),
        about: kept::about_from_json(&about)?,
        comment: kept::comment_from_json(&comment)?,
        written_by: kept::agent_from_text(&written_by)?,
        written_at: kept::from_microseconds(written_at)?,
        submitted_at: submitted_at.map(kept::from_microseconds).transpose()?,
        edited: row.try_get("edited").map_err(unreachable)?,
        done: row.try_get("done").map_err(unreachable)?,
    })
}

fn all_catalogued(rows: &[SqliteRow]) -> Result<Vec<CataloguedFeedback>, CatalogError> {
    rows.iter().map(catalogued).collect()
}

#[async_trait]
impl FeedbackCatalog for SqliteFeedbackCatalog {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        piece: &CataloguedFeedback,
    ) -> Result<(), CatalogError> {
        let (subject_kind, subject_id) = kept::subject_columns(&piece.about.subject());
        let (place_kind, place_id) = kept::place_columns(&piece.about);
        sqlx::query(KEEP)
            .bind(piece.id.to_string())
            .bind(piece.project.as_str())
            .bind(subject_kind)
            .bind(subject_id)
            .bind(place_kind)
            .bind(place_id)
            .bind(kept::about_to_json(&piece.about))
            .bind(kept::comment_to_json(&piece.comment))
            .bind(kept::agent_to_text(&piece.written_by))
            .bind(kept::in_microseconds(piece.written_at))
            .bind(piece.submitted_at.map(kept::in_microseconds))
            .bind(piece.edited)
            .bind(piece.done)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        giver_found_again(
            transaction,
            piece.project.as_str(),
            &kept::agent_to_text(&piece.written_by),
        )
        .await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        piece: &FeedbackId,
    ) -> Result<(), CatalogError> {
        let forgotten = sqlx::query(FORGET)
            .bind(piece.to_string())
            .fetch_optional(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        let Some(forgotten) = forgotten else {
            return Ok(());
        };
        let project: String = forgotten.try_get("project").map_err(unreachable)?;
        let giver: String = forgotten.try_get("written_by").map_err(unreachable)?;

        giver_found_again(transaction, &project, &giver).await
    }

    async fn piece(&self, piece: &FeedbackId) -> Result<Option<CataloguedFeedback>, CatalogError> {
        sqlx::query(AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM feedback WHERE feedback = ?1"
        )))
        .bind(piece.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(unreachable)?
        .as_ref()
        .map(catalogued)
        .transpose()
    }

    async fn drafts_of(
        &self,
        asked: &Asked,
        writer: &Agent,
    ) -> Result<Vec<CataloguedFeedback>, CatalogError> {
        let sql = format!(
            "SELECT {COLUMNS} FROM feedback
             WHERE {ASKED} AND submitted_at IS NULL AND written_by = ?5
             ORDER BY written_at, feedback"
        );
        let rows = asking(sql, asked)
            .bind(kept::agent_to_text(writer))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        all_catalogued(&rows)
    }

    async fn submitted(
        &self,
        asked: &Asked,
        status: Option<Status>,
        page: &Page,
    ) -> Result<Vec<CataloguedFeedback>, CatalogError> {
        let (onwards, direction) = match page.order {
            Order::Oldest => (">", "ASC"),
            Order::Newest => ("<", "DESC"),
        };
        let sql = format!(
            "SELECT {COLUMNS} FROM feedback
             WHERE {ASKED} AND submitted_at IS NOT NULL
               AND (?5 IS NULL OR done = ?5)
               AND (?6 IS NULL
                    OR NOT EXISTS (SELECT 1 FROM feedback after WHERE after.feedback = ?6)
                    OR (submitted_at, written_at, feedback) {onwards}
                       (SELECT after.submitted_at, after.written_at, after.feedback
                        FROM feedback after WHERE after.feedback = ?6))
             ORDER BY submitted_at {direction}, written_at {direction}, feedback {direction}
             LIMIT COALESCE(?7, -1)"
        );
        let rows = asking(sql, asked)
            .bind(status.map(|status| status == Status::Done))
            .bind(page.after.clone())
            .bind(
                page.at_most
                    .map(|most| i64::try_from(most).unwrap_or(i64::MAX)),
            )
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        all_catalogued(&rows)
    }

    async fn pending(&self, asked: &Asked) -> Result<Pending, CatalogError> {
        let sql = format!(
            "SELECT place_kind, place_id FROM feedback
             WHERE {ASKED} AND submitted_at IS NOT NULL AND NOT done"
        );
        let rows = asking(sql, asked)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        let mut pending = Pending::default();
        for row in rows {
            let kind: Option<String> = row.try_get("place_kind").map_err(unreachable)?;
            let id: Option<String> = row.try_get("place_id").map_err(unreachable)?;
            match (kind.as_deref(), id) {
                (Some("scene"), Some(scene)) => {
                    *pending.scenes.entry(scene.into()).or_insert(0) += 1;
                }
                (Some("section"), Some(section)) => {
                    *pending.sections.entry(section.into()).or_insert(0) += 1;
                }
                _ => {}
            }
            pending.total += 1;
        }

        Ok(pending)
    }

    async fn drafts_on(
        &self,
        subject: &Subject,
        at_most: usize,
    ) -> Result<Vec<FeedbackId>, CatalogError> {
        let (kind, id) = kept::subject_columns(subject);

        sqlx::query(DRAFTS_ON)
            .bind(kind)
            .bind(id)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row| {
                let piece: String = row.try_get("feedback").map_err(unreachable)?;

                FeedbackId::from_str(&piece).map_err(unreachable)
            })
            .collect()
    }

    async fn pieces_on(
        &self,
        subject: &Subject,
        at_most: usize,
    ) -> Result<Vec<FeedbackId>, CatalogError> {
        let (kind, id) = kept::subject_columns(subject);

        sqlx::query(PIECES_ON)
            .bind(kind)
            .bind(id)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row| {
                let piece: String = row.try_get("feedback").map_err(unreachable)?;

                FeedbackId::from_str(&piece).map_err(unreachable)
            })
            .collect()
    }

    async fn givers_in(&self, project: &ProjectLink) -> Result<Vec<Agent>, CatalogError> {
        sqlx::query(GIVERS_IN)
            .bind(project.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row| {
                let giver: String = row.try_get("giver").map_err(unreachable)?;

                kept::agent_from_text(&giver)
            })
            .collect()
    }

    async fn counted_on(
        &self,
        subjects: &[Subject],
        asker: &Agent,
    ) -> Result<Vec<Counted>, CatalogError> {
        let asked = as_json(
            &subjects
                .iter()
                .map(kept::subject_columns)
                .map(|(kind, id)| [kind, id])
                .collect::<Vec<[String; 2]>>(),
        );
        let mut counts: HashMap<(String, String), (usize, usize)> = HashMap::new();
        for row in sqlx::query(COUNTED_ON)
            .bind(asked)
            .bind(kept::agent_to_text(asker))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
        {
            let kind: String = row.try_get("subject_kind").map_err(unreachable)?;
            let id: String = row.try_get("subject_id").map_err(unreachable)?;
            let not_done: i64 = row.try_get("not_done").map_err(unreachable)?;
            let your_drafts: i64 = row.try_get("your_drafts").map_err(unreachable)?;
            counts.insert(
                (kind, id),
                (
                    usize::try_from(not_done).map_err(unreachable)?,
                    usize::try_from(your_drafts).map_err(unreachable)?,
                ),
            );
        }

        Ok(subjects
            .iter()
            .map(|subject| {
                let (not_done, your_drafts) = counts
                    .get(&kept::subject_columns(subject))
                    .copied()
                    .unwrap_or_default();

                Counted {
                    subject: subject.clone(),
                    not_done,
                    your_drafts,
                }
            })
            .collect())
    }
}
