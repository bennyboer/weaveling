use std::sync::Arc;

use crate::compaction::COMPACT_AFTER;
use crate::enqueuing::{Mapping, SceneMessageMapping};
use async_trait::async_trait;
use clock::Clock;
use outbox::Origin;
use outbox::postgres::enqueue;
use scenes_core::{
    IdeaLink, ProjectLink, Scene, SceneChange, SceneId, SceneStore, SceneTitle, StoreError,
};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Postgres, Row, Transaction};

const REMEMBER: &str = "INSERT INTO scenes (scene, project, title) VALUES ($1, $2, $3)";

const KNOWN_AS: &str = "SELECT project, title FROM scenes WHERE scene = $1";

const RETITLE: &str = "UPDATE scenes SET title = $2, version = version + 1 WHERE scene = $1";

const LINK: &str = "
    INSERT INTO scene_ideas (scene, idea)
    VALUES ($1, $2)
    ON CONFLICT (scene, idea) DO NOTHING
";

const UNLINK: &str = "DELETE FROM scene_ideas WHERE scene = $1 AND idea = $2";

const BUMP: &str = "UPDATE scenes SET version = version + 1 WHERE scene = $1 RETURNING version";

const UNLINK_EVERYWHERE: &str = "DELETE FROM scene_ideas WHERE idea = $1";

const LINKED_IDEAS: &str = "SELECT idea FROM scene_ideas WHERE scene = $1 ORDER BY seq";

const EXISTS: &str = "SELECT 1 FROM scenes WHERE scene = $1";

const IN_PROJECT: &str = "
    SELECT scene
    FROM scenes
    WHERE project = $1 AND scene > $2
    ORDER BY scene
    LIMIT $3
";

const WRITE_SNAPSHOT: &str = "
    INSERT INTO scene_updates (scene, is_snapshot, bytes)
    VALUES ($1, TRUE, $2)
    RETURNING seq
";

const WRITE_UPDATE: &str = "
    WITH bumped AS (
        UPDATE scenes SET version = version + 1 WHERE scene = $1
    )
    INSERT INTO scene_updates (scene, is_snapshot, bytes)
    VALUES ($1, FALSE, $2)
    RETURNING (
        SELECT count(*)
        FROM scene_updates behind
        WHERE behind.scene = $1 AND behind.seq > COALESCE((
            SELECT max(newest.seq)
            FROM scene_updates newest
            WHERE newest.scene = $1 AND newest.is_snapshot
        ), 0)
    ) AS tail
";

const READ_SINCE_SNAPSHOT: &str = "
    SELECT bytes
    FROM scene_updates
    WHERE scene = $1 AND seq >= COALESCE((
        SELECT max(seq)
        FROM scene_updates
        WHERE scene = $1 AND is_snapshot
    ), 0)
    ORDER BY seq
";

const HOLD: &str = "SELECT project FROM scenes WHERE scene = $1 FOR UPDATE";

const FORGET_BEFORE: &str = "DELETE FROM scene_updates WHERE scene = $1 AND seq < $2";

const FORGET: &str = "DELETE FROM scenes WHERE scene = $1";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/postgres")
}

const KIND: &str = "scene";

pub struct PostgresSceneStore {
    pool: PgPool,
    compact_after: i64,
    enqueuing: Option<Mapping>,
}

impl PostgresSceneStore {
    pub fn new(pool: PgPool) -> Self {
        Self::compacting_after(pool, COMPACT_AFTER)
    }

    pub fn compacting_after(pool: PgPool, updates: i64) -> Self {
        Self {
            pool,
            compact_after: updates,
            enqueuing: None,
        }
    }

    pub fn enqueuing(self, clock: Arc<dyn Clock>, message_for: SceneMessageMapping) -> Self {
        Self {
            enqueuing: Some(Mapping::new(clock, message_for)),
            ..self
        }
    }

    async fn enqueue_change(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        change: SceneChange,
    ) -> Result<(), StoreError> {
        let Some(mapping) = &self.enqueuing else {
            return Ok(());
        };
        let scene = change.scene().to_string();
        let origin = Origin {
            aggregate: &scene,
            kind: KIND,
            version: 0,
        };

        enqueue(transaction, origin, &mapping.message(&change))
            .await
            .map_err(unreachable)
    }

    pub async fn compact(&self, id: SceneId) -> Result<bool, StoreError> {
        let mut transaction = self.begin().await?;

        let found: Option<String> = sqlx::query_scalar(HOLD)
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(unreachable)?;
        let Some(project) = found else {
            return Err(StoreError::NotFound(id));
        };

        let parts = self.parts(&mut transaction, id).await?;
        if parts.len() as i64 <= self.compact_after {
            return Ok(false);
        }

        let scene = grown(id, ProjectLink::from(project), &parts)?;
        let collapsed: i64 = sqlx::query_scalar(WRITE_SNAPSHOT)
            .bind(id.to_string())
            .bind(scene.everything())
            .fetch_one(&mut *transaction)
            .await
            .map_err(unreachable)?;

        sqlx::query(FORGET_BEFORE)
            .bind(id.to_string())
            .bind(collapsed)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)?;

        Ok(true)
    }

    async fn begin(&self) -> Result<Transaction<'_, Postgres>, StoreError> {
        self.pool.begin().await.map_err(unreachable)
    }

    async fn parts(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        id: SceneId,
    ) -> Result<Vec<Vec<u8>>, StoreError> {
        let found = sqlx::query(READ_SINCE_SNAPSHOT)
            .bind(id.to_string())
            .fetch_all(&mut **transaction)
            .await
            .map_err(unreachable)?;

        Ok(found
            .iter()
            .map(|row| row.get::<Vec<u8>, _>("bytes"))
            .collect())
    }
}

fn grown(id: SceneId, project: ProjectLink, parts: &[Vec<u8>]) -> Result<Scene, StoreError> {
    let scene = Scene::empty(id, project);

    for part in parts {
        scene
            .apply(part)
            .map_err(|reason| StoreError::Backend(Box::new(reason)))?;
    }

    Ok(scene)
}

async fn bumped(
    transaction: &mut Transaction<'_, Postgres>,
    id: SceneId,
) -> Result<u64, StoreError> {
    let version: i64 = sqlx::query_scalar(BUMP)
        .bind(id.to_string())
        .fetch_one(&mut **transaction)
        .await
        .map_err(unreachable)?;

    Ok(u64::try_from(version).expect("a version counts up from zero"))
}

fn unreachable(failure: sqlx::Error) -> StoreError {
    StoreError::Backend(Box::new(failure))
}

fn is_taken(failure: &sqlx::Error) -> bool {
    matches!(failure, sqlx::Error::Database(reason) if reason.is_unique_violation())
}

fn is_unknown(failure: &sqlx::Error) -> bool {
    matches!(failure, sqlx::Error::Database(reason) if reason.is_foreign_key_violation())
}

#[async_trait]
impl SceneStore for PostgresSceneStore {
    async fn create(&self, scene: &Scene) -> Result<(), StoreError> {
        let id = scene.id();
        let mut transaction = self.begin().await?;

        sqlx::query(REMEMBER)
            .bind(id.to_string())
            .bind(scene.project().to_string())
            .bind(scene.title().as_str())
            .execute(&mut *transaction)
            .await
            .map_err(|failure| match is_taken(&failure) {
                true => StoreError::Conflict(id),
                false => unreachable(failure),
            })?;

        sqlx::query(WRITE_SNAPSHOT)
            .bind(id.to_string())
            .bind(scene.everything())
            .fetch_one(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn load(&self, id: SceneId) -> Result<Scene, StoreError> {
        let found = sqlx::query(READ_SINCE_SNAPSHOT)
            .bind(id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        if found.is_empty() {
            return Err(StoreError::NotFound(id));
        }

        let parts: Vec<Vec<u8>> = found
            .iter()
            .map(|row| row.get::<Vec<u8>, _>("bytes"))
            .collect();

        let (project, title): (String, String) = sqlx::query_as(KNOWN_AS)
            .bind(id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;
        let title = SceneTitle::new(&title).map_err(|why| StoreError::Backend(Box::new(why)))?;

        let ideas: Vec<String> = sqlx::query_scalar(LINKED_IDEAS)
            .bind(id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(grown(id, ProjectLink::from(project), &parts)?
            .titled(title)
            .linked_to(ideas.into_iter().map(IdeaLink::from).collect()))
    }

    async fn apply(&self, id: SceneId, update: &[u8]) -> Result<(), StoreError> {
        Scene::readable(update).map_err(|_| StoreError::Unusable(id))?;

        let tail: i64 = sqlx::query_scalar(WRITE_UPDATE)
            .bind(id.to_string())
            .bind(update)
            .fetch_one(&self.pool)
            .await
            .map_err(|failure| match is_unknown(&failure) {
                true => StoreError::NotFound(id),
                false => unreachable(failure),
            })?;

        if tail + 1 > self.compact_after {
            self.compact(id).await?;
        }

        Ok(())
    }

    async fn retitle(&self, id: SceneId, title: &SceneTitle) -> Result<(), StoreError> {
        let changed = sqlx::query(RETITLE)
            .bind(id.to_string())
            .bind(title.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        match changed.rows_affected() {
            0 => Err(StoreError::NotFound(id)),
            _ => Ok(()),
        }
    }

    async fn link(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut transaction = self.begin().await?;

        let linked = sqlx::query(LINK)
            .bind(id.to_string())
            .bind(idea.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(|failure| match is_unknown(&failure) {
                true => StoreError::NotFound(id),
                false => unreachable(failure),
            })?;

        if linked.rows_affected() > 0 {
            let version = bumped(&mut transaction, id).await?;
            self.enqueue_change(
                &mut transaction,
                SceneChange::IdeaLinked {
                    scene: id,
                    idea: idea.clone(),
                    version,
                },
            )
            .await?;
        }

        transaction.commit().await.map_err(unreachable)
    }

    async fn unlink(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut transaction = self.begin().await?;

        let known: Option<i32> = sqlx::query_scalar(EXISTS)
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(unreachable)?;
        if known.is_none() {
            return Err(StoreError::NotFound(id));
        }

        let unlinked = sqlx::query(UNLINK)
            .bind(id.to_string())
            .bind(idea.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        if unlinked.rows_affected() > 0 {
            let version = bumped(&mut transaction, id).await?;
            self.enqueue_change(
                &mut transaction,
                SceneChange::IdeaUnlinked {
                    scene: id,
                    idea: idea.clone(),
                    version,
                },
            )
            .await?;
        }

        transaction.commit().await.map_err(unreachable)
    }

    async fn unlink_everywhere(&self, idea: &IdeaLink) -> Result<(), StoreError> {
        sqlx::query(UNLINK_EVERYWHERE)
            .bind(idea.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, StoreError> {
        let found: Vec<String> = sqlx::query_scalar(IN_PROJECT)
            .bind(project.to_string())
            .bind(after.map(|last| last.to_string()).unwrap_or_default())
            .bind(at_most as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|scene| {
                scene
                    .parse()
                    .map_err(|why| StoreError::Backend(Box::new(why)))
            })
            .collect()
    }

    async fn delete(&self, id: SceneId) -> Result<(), StoreError> {
        let mut transaction = self.begin().await?;

        let gone = sqlx::query(FORGET)
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        if gone.rows_affected() == 0 {
            return Err(StoreError::NotFound(id));
        }

        self.enqueue_change(&mut transaction, SceneChange::Deleted { scene: id })
            .await?;

        transaction.commit().await.map_err(unreachable)
    }
}
