use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use eventsourcing::Agent;
use feedback_core::{
    Asked, CatalogError, CataloguedFeedback, Counted, FeedbackCatalog, FeedbackId, Order, Page,
    Pending, Places, ProjectLink, Status, Subject,
};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::Query;
use sqlx::{AssertSqlSafe, PgPool, Postgres, Row};
use transacting::Transaction;
use transacting::postgres::connection;

use crate::kept;
use crate::postgres::unreachable;

const COLUMNS: &str =
    "feedback, project, about, comment, written_by, written_at, submitted_at, edited, done";

const KEEP: &str = "
    INSERT INTO feedback (feedback, project, subject_kind, subject_id, place_kind, place_id, about,
                          comment, written_by, written_at, submitted_at, edited, done)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
    ON CONFLICT (feedback) DO UPDATE
    SET project = excluded.project, subject_kind = excluded.subject_kind,
        subject_id = excluded.subject_id, place_kind = excluded.place_kind,
        place_id = excluded.place_id, about = excluded.about, comment = excluded.comment,
        written_by = excluded.written_by, written_at = excluded.written_at,
        submitted_at = excluded.submitted_at, edited = excluded.edited, done = excluded.done
";

const FORGET: &str = "DELETE FROM feedback WHERE feedback = $1 RETURNING project, written_by";

/// Which pieces are asked for, as the first seven parameters name them: the project;
/// the subjects' kinds and ids, none if not asked by; whether places are named, and their kinds and ids; and the givers.
/// With no subjects named it is feedback on the work alone, as replies are asked for by the piece they are given on.
const ASKED: &str = "
    ($1::TEXT IS NULL OR project = $1)
    AND (($2::TEXT[] IS NULL AND subject_kind <> 'feedback')
         OR (subject_kind, subject_id) IN (SELECT * FROM UNNEST($2::TEXT[], $3::TEXT[])))
    AND (NOT $4::BOOLEAN
         OR (place_kind, place_id) IN (SELECT * FROM UNNEST($5::TEXT[], $6::TEXT[])))
    AND ($7::TEXT[] IS NULL OR written_by = ANY($7))
";

const GIVERS_IN: &str =
    "SELECT giver FROM givers WHERE project = $1 ORDER BY submitted_at, written_at, feedback";

/// A giver's row made again from their first piece submitted in the project, or gone with the last of them.
const FORGET_GIVER: &str = "DELETE FROM givers WHERE project = $1 AND giver = $2";

const FIND_GIVER: &str = "
    INSERT INTO givers (project, giver, submitted_at, written_at, feedback)
    SELECT project, written_by, submitted_at, written_at, feedback
    FROM feedback
    WHERE project = $1 AND written_by = $2 AND submitted_at IS NOT NULL
      AND subject_kind <> 'feedback'
    ORDER BY submitted_at, written_at, feedback
    LIMIT 1
";

const DRAFTS_ON: &str = "SELECT feedback FROM feedback WHERE subject_kind = $1 AND subject_id = $2 AND submitted_at IS NULL ORDER BY feedback LIMIT $3";

const PIECES_ON: &str = "SELECT feedback FROM feedback WHERE subject_kind = $1 AND subject_id = $2 ORDER BY feedback LIMIT $3";

const COUNTED_ON: &str = "
    SELECT subject_kind, subject_id,
           COUNT(*) FILTER (WHERE submitted_at IS NOT NULL) AS given,
           COUNT(*) FILTER (WHERE submitted_at IS NOT NULL AND NOT done) AS not_done,
           COUNT(*) FILTER (WHERE submitted_at IS NULL AND written_by = $3) AS your_drafts
    FROM feedback
    WHERE (subject_kind, subject_id) IN (SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[]))
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

pub struct PostgresFeedbackCatalog {
    pool: PgPool,
}

impl PostgresFeedbackCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// A query with the pieces asked for bound as its first seven parameters.
/// The SQL is built only from this module's constants, so it holds nothing anyone asked with.
fn asking(sql: String, asked: &Asked) -> Query<'static, Postgres, PgArguments> {
    let (subject_kinds, subject_ids): (Vec<String>, Vec<String>) = asked
        .subjects
        .iter()
        .flatten()
        .map(kept::subject_columns)
        .unzip();
    let (place_kinds, place_ids): (Vec<String>, Vec<String>) = match &asked.places {
        Some(Places::Text { scenes, sections }) => scenes
            .iter()
            .map(|scene| ("scene".to_owned(), scene.to_string()))
            .chain(
                sections
                    .iter()
                    .map(|section| ("section".to_owned(), section.to_string())),
            )
            .unzip(),
        None => (Vec::new(), Vec::new()),
    };

    sqlx::query(AssertSqlSafe(sql))
        .bind(asked.project.clone())
        .bind(asked.subjects.is_some().then_some(subject_kinds))
        .bind(subject_ids)
        .bind(asked.places.is_some())
        .bind(place_kinds)
        .bind(place_ids)
        .bind(asked.given_by.as_ref().map(|givers| {
            givers
                .iter()
                .map(kept::agent_to_text)
                .collect::<Vec<String>>()
        }))
}

fn catalogued(row: &PgRow) -> Result<CataloguedFeedback, CatalogError> {
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

fn all_catalogued(rows: &[PgRow]) -> Result<Vec<CataloguedFeedback>, CatalogError> {
    rows.iter().map(catalogued).collect()
}

#[async_trait]
impl FeedbackCatalog for PostgresFeedbackCatalog {
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
            "SELECT {COLUMNS} FROM feedback WHERE feedback = $1"
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
             WHERE {ASKED} AND submitted_at IS NULL AND written_by = $8
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
               AND ($8::BOOLEAN IS NULL OR done = $8)
               AND ($9::TEXT IS NULL
                    OR NOT EXISTS (SELECT 1 FROM feedback after WHERE after.feedback = $9)
                    OR (submitted_at, written_at, feedback) {onwards}
                       (SELECT after.submitted_at, after.written_at, after.feedback
                        FROM feedback after WHERE after.feedback = $9))
             ORDER BY submitted_at {direction}, written_at {direction}, feedback {direction}
             LIMIT $10"
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
        let (kinds, ids): (Vec<String>, Vec<String>) =
            subjects.iter().map(kept::subject_columns).unzip();
        let mut counts: HashMap<(String, String), (usize, usize, usize)> = HashMap::new();
        for row in sqlx::query(COUNTED_ON)
            .bind(&kinds)
            .bind(&ids)
            .bind(kept::agent_to_text(asker))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
        {
            let kind: String = row.try_get("subject_kind").map_err(unreachable)?;
            let id: String = row.try_get("subject_id").map_err(unreachable)?;
            let given: i64 = row.try_get("given").map_err(unreachable)?;
            let not_done: i64 = row.try_get("not_done").map_err(unreachable)?;
            let your_drafts: i64 = row.try_get("your_drafts").map_err(unreachable)?;
            counts.insert(
                (kind, id),
                (
                    usize::try_from(given).map_err(unreachable)?,
                    usize::try_from(not_done).map_err(unreachable)?,
                    usize::try_from(your_drafts).map_err(unreachable)?,
                ),
            );
        }

        Ok(subjects
            .iter()
            .map(|subject| {
                let (given, not_done, your_drafts) = counts
                    .get(&kept::subject_columns(subject))
                    .copied()
                    .unwrap_or_default();

                Counted {
                    subject: subject.clone(),
                    given,
                    not_done,
                    your_drafts,
                }
            })
            .collect())
    }
}
