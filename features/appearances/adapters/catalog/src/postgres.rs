use appearances_core::{AppearanceCatalog, CatalogError, PassageLink, Place, SectionLink, Subject};
use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

const IDEA: &str = "idea";

const PASSAGE: &str = "passage";
const SECTION: &str = "section";

const REMEMBER: &str = "
    INSERT INTO appearances (subject_type, subject_id, place_type, place_id)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT DO NOTHING
";

const FORGET: &str = "
    DELETE FROM appearances
    WHERE subject_type = $1 AND subject_id = $2 AND place_type = $3 AND place_id = $4
";

const FORGET_SUBJECT: &str = "DELETE FROM appearances WHERE subject_type = $1 AND subject_id = $2";

const FORGET_PLACE: &str = "DELETE FROM appearances WHERE place_type = $1 AND place_id = $2";

const PLACES_OF: &str = "
    SELECT place_type, place_id
    FROM appearances
    WHERE subject_type = $1 AND subject_id = $2
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

fn place_from(row: &PgRow) -> Result<Place, CatalogError> {
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
impl AppearanceCatalog for PostgresAppearanceCatalog {
    async fn remember(&self, subject: &Subject, place: &Place) -> Result<(), CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);
        let (place_type, place_id) = place_type_and_id(place);

        sqlx::query(REMEMBER)
            .bind(subject_type)
            .bind(subject_id)
            .bind(place_type)
            .bind(place_id)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(&self, subject: &Subject, place: &Place) -> Result<(), CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);
        let (place_type, place_id) = place_type_and_id(place);

        sqlx::query(FORGET)
            .bind(subject_type)
            .bind(subject_id)
            .bind(place_type)
            .bind(place_id)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_subject(&self, subject: &Subject) -> Result<(), CatalogError> {
        let (subject_type, subject_id) = subject_type_and_id(subject);

        sqlx::query(FORGET_SUBJECT)
            .bind(subject_type)
            .bind(subject_id)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_place(&self, place: &Place) -> Result<(), CatalogError> {
        let (place_type, place_id) = place_type_and_id(place);

        sqlx::query(FORGET_PLACE)
            .bind(place_type)
            .bind(place_id)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
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
