mod folder;
mod memory;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

pub use folder::FolderContents;
pub use memory::InMemoryContents;

#[cfg(feature = "postgres")]
pub use postgres::PostgresContents;

use assets_core::ContentsError;

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> ContentsError {
    ContentsError::Backend(Box::new(failure))
}
