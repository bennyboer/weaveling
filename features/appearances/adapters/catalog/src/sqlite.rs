use appearances_core::{AppearanceCatalog, CatalogError, PassageLink, Place, SectionLink, Subject};
use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

const IDEA: &str = "idea";

const PASSAGE: &str = "passage";
const SECTION: &str = "section";

const MARK: &str = "
    INSERT INTO appearances (subject_type, subject_id, place_type, place_id, version, present)
    SELECT ?1, ?2, ?3, ?4, ?5, ?6
    WHERE NOT EXISTS (
            SELECT 1 FROM gone_subjects WHERE subject_type = ?1 AND subject_id = ?2
        )
        AND NOT EXISTS (
            SELECT 1 FROM gone_places WHERE place_type = ?3 AND place_id = ?4
        )
    ON CONFLICT (subject_type, subject_id, place_type, place_id) DO UPDATE
    SET version = excluded.version, present = excluded.present
    WHERE appearances.version <= excluded.version
";

const GONE_SUBJECT: &str = "
    INSERT INTO gone_subjects (subject_type, subject_id)
    VALUES (?1, ?2)
    ON CONFLICT DO NOTHING
";

const FORGET_SUBJECT: &str = "DELETE FROM appearances WHERE subject_type = ?1 AND subject_id = ?2";

const GONE_PLACE: &str = "
    INSERT INTO gone_places (place_type, place_id)
    VALUES (?1, ?2)
    ON CONFLICT DO NOTHING
";

const FORGET_PLACE: &str = "DELETE FROM appearances WHERE place_type = ?1 AND place_id = ?2";

const BEGIN_WRITING: &str = "BEGIN IMMEDIATE";

const PLACES_OF: &str = "
    SELECT place_type, place_id
    FROM appearances
    WHERE subject_type = ?1 AND subject_id = ?2 AND present
    ORDER BY place_type, place_id
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub struct SqliteAppearanceCatalog {
    pool: SqlitePool,
}

impl SqliteAppearanceCatalog {
    pub fn new(pool: SqlitePool) -> Self {
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
        Place::Passage(passage) => (PASSAGE, passage.as_str()),
        Place::Section(section) => (SECTION, section.as_str()),
    }
}

fn place_from(row: &SqliteRow) -> Result<Place, CatalogError> {
    let place_type: String = row.try_get("place_type").map_err(unreachable)?;
    let place_id: String = row.try_get("place_id").map_err(unreachable)?;

    match place_type.as_str() {
        PASSAGE => Ok(Place::Passage(PassageLink::from(place_id))),
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
impl AppearanceCatalog for SqliteAppearanceCatalog {
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

    async fn forget_subject(&self, subject: &Subject) -> Result<(), CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);
        let mut transaction = self
            .pool
            .begin_with(BEGIN_WRITING)
            .await
            .map_err(unreachable)?;

        for gone in [GONE_SUBJECT, FORGET_SUBJECT] {
            sqlx::query(gone)
                .bind(subject_type)
                .bind(subject_id)
                .execute(&mut *transaction)
                .await
                .map_err(unreachable)?;
        }

        transaction.commit().await.map_err(unreachable)
    }

    async fn forget_place(&self, place: &Place) -> Result<(), CatalogError> {
        let (place_type, place_id) = place_type_and_id(place);
        let mut transaction = self
            .pool
            .begin_with(BEGIN_WRITING)
            .await
            .map_err(unreachable)?;

        for gone in [GONE_PLACE, FORGET_PLACE] {
            sqlx::query(gone)
                .bind(place_type)
                .bind(place_id)
                .execute(&mut *transaction)
                .await
                .map_err(unreachable)?;
        }

        transaction.commit().await.map_err(unreachable)
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
}
