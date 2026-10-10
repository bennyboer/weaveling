mod compaction;
mod enqueuing;
mod memory;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

#[cfg(test)]
mod suite;

pub use compaction::COMPACT_AFTER;
pub use enqueuing::SceneMessageMapping;
pub use memory::InMemorySceneStore;

#[cfg(feature = "postgres")]
pub use postgres::PostgresSceneStore;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteSceneStore;
