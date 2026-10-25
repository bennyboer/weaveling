use appearances_core::{
    AppearanceCatalog, CatalogError, IdeaLink, Place, ProjectLink, SceneLink, SectionLink, Subject,
};
use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

const IDEA: &str = "idea";

const SCENE: &str = "scene";
const SECTION: &str = "section";

const MARK: &str = "
    INSERT INTO appearances (subject_type, subject_id, place_type, place_id, version, present)
    SELECT $1, $2, $3, $4, $5, $6
    WHERE NOT EXISTS (
            SELECT 1 FROM gone_subjects WHERE subject_type = $1 AND subject_id = $2
        )
        AND NOT EXISTS (
            SELECT 1 FROM gone_places WHERE place_type = $3 AND place_id = $4
        )
    ON CONFLICT (subject_type, subject_id, place_type, place_id) DO UPDATE
    SET version = excluded.version, present = excluded.present
    WHERE appearances.version <= excluded.version
";

const GONE_SUBJECT: &str = "
    INSERT INTO gone_subjects (subject_type, subject_id, gone_at)
    VALUES ($1, $2, $3)
    ON CONFLICT DO NOTHING
";

const FORGET_SUBJECT: &str = "DELETE FROM appearances WHERE subject_type = $1 AND subject_id = $2";

const GONE_PLACE: &str = "
    INSERT INTO gone_places (place_type, place_id, gone_at)
    VALUES ($1, $2, $3)
    ON CONFLICT DO NOTHING
";

const FORGET_PLACE: &str = "
    DELETE FROM appearances WHERE place_type = $1 AND place_id = $2
    RETURNING subject_type, subject_id, present
";

const REMEMBER_PROJECT: &str = "
    INSERT INTO subject_projects (subject_type, subject_id, project)
    VALUES ($1, $2, $3)
    ON CONFLICT (subject_type, subject_id) DO UPDATE SET project = excluded.project
";

const FORGET_LONG_GONE_SUBJECTS: &str = "
    WITH long_gone AS (
        DELETE FROM gone_subjects
        WHERE (subject_type, subject_id) IN (
            SELECT subject_type, subject_id FROM gone_subjects
            WHERE gone_at < $1
            ORDER BY gone_at
            LIMIT $2
        )
        RETURNING subject_type, subject_id
    ), projects_of_long_gone AS (
        DELETE FROM subject_projects
        USING long_gone
        WHERE subject_projects.subject_type = long_gone.subject_type
            AND subject_projects.subject_id = long_gone.subject_id
    )
    SELECT COUNT(*) FROM long_gone
";

const FORGET_LONG_GONE_PLACES: &str = "
    DELETE FROM gone_places
    WHERE (place_type, place_id) IN (
        SELECT place_type, place_id FROM gone_places
        WHERE gone_at < $1
        ORDER BY gone_at
        LIMIT $2
    )
";

const PROJECT_OF: &str =
    "SELECT project FROM subject_projects WHERE subject_type = $1 AND subject_id = $2";

const PLACES_OF: &str = "
    SELECT place_type, place_id
    FROM appearances
    WHERE subject_type = $1 AND subject_id = $2 AND present
    ORDER BY place_type, place_id
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/postgres")
}

pub struct PostgresAppearanceCatalog {
    pool: PgPool,
}

impl PostgresAppearanceCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    async fn mark(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
        present: bool,
    ) -> Result<(), CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);
        let (place_type, place_id) = place_type_and_id(place);

        sqlx::query(MARK)
            .bind(subject_type)
            .bind(subject_id)
            .bind(place_type)
            .bind(place_id)
            .bind(i64::try_from(version).expect("a version fits a signed 64-bit column"))
            .bind(present)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

fn subject_type_and_id(subject: &Subject) -> (&'static str, &str) {
    match subject {
        Subject::Idea(idea) => (IDEA, idea.as_str()),
    }
}

fn place_type_and_id(place: &Place) -> (&'static str, &str) {
    match place {
        Place::Scene(scene) => (SCENE, scene.as_str()),
        Place::Section(section) => (SECTION, section.as_str()),
    }
}

fn appeared_from(row: &PgRow) -> Result<Option<Subject>, CatalogError> {
    let subject_type: String = row.try_get("subject_type").map_err(unreachable)?;
    let subject_id: String = row.try_get("subject_id").map_err(unreachable)?;
    let present: bool = row.try_get("present").map_err(unreachable)?;

    match subject_type.as_str() {
        _ if !present => Ok(None),
        IDEA => Ok(Some(Subject::Idea(IdeaLink::from(subject_id)))),
        unknown => Err(CatalogError::Backend(
            format!("an appearance of a type of subject nothing writes: {unknown}").into(),
        )),
    }
}

fn place_from(row: &PgRow) -> Result<Place, CatalogError> {
    let place_type: String = row.try_get("place_type").map_err(unreachable)?;
    let place_id: String = row.try_get("place_id").map_err(unreachable)?;

    match place_type.as_str() {
        SCENE => Ok(Place::Scene(SceneLink::from(place_id))),
        SECTION => Ok(Place::Section(SectionLink::from(place_id))),
        unknown => Err(CatalogError::Backend(
            format!("an appearance at a type of place nothing writes: {unknown}").into(),
        )),
    }
}

fn unreachable(failure: sqlx::Error) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl AppearanceCatalog for PostgresAppearanceCatalog {
    async fn remember(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.mark(subject, place, version, true).await
    }

    async fn forget(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.mark(subject, place, version, false).await
    }

    async fn forget_subject(
        &self,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(GONE_SUBJECT)
            .bind(subject_type)
            .bind(subject_id)
            .bind(at)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET_SUBJECT)
            .bind(subject_type)
            .bind(subject_id)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn forget_place(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError> {
        let (place_type, place_id) = place_type_and_id(place);
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(GONE_PLACE)
            .bind(place_type)
            .bind(place_id)
            .bind(at)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        let forgotten = sqlx::query(FORGET_PLACE)
            .bind(place_type)
            .bind(place_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(unreachable)?;
        transaction.commit().await.map_err(unreachable)?;

        let mut appeared = Vec::new();
        for row in &forgotten {
            appeared.extend(appeared_from(row)?);
        }
        appeared.sort();

        Ok(appeared)
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let at_most = i64::try_from(at_most).unwrap_or(i64::MAX);

        let subjects: i64 = sqlx::query_scalar(FORGET_LONG_GONE_SUBJECTS)
            .bind(before)
            .bind(at_most)
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;
        let places = sqlx::query(FORGET_LONG_GONE_PLACES)
            .bind(before)
            .bind(at_most)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?
            .rows_affected();

        Ok(u64::try_from(subjects).expect("a count is never negative") + places)
    }

    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);

        sqlx::query(PLACES_OF)
            .bind(subject_type)
            .bind(subject_id)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(place_from)
            .collect()
    }

    async fn remember_project(
        &self,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);

        sqlx::query(REMEMBER_PROJECT)
            .bind(subject_type)
            .bind(subject_id)
            .bind(project.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn project_of(&self, subject: &Subject) -> Result<Option<ProjectLink>, CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);

        let kept: Option<String> = sqlx::query_scalar(PROJECT_OF)
            .bind(subject_type)
            .bind(subject_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(kept.map(ProjectLink::from))
    }
}
