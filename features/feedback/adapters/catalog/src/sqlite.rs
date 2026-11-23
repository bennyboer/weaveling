use feedback_core::CatalogError;
use sqlx::migrate::Migrator;

/// The feedback feature's tables on SQLite: the feedback, the reading copies heard of, and the givers' names.
pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub(crate) fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}
