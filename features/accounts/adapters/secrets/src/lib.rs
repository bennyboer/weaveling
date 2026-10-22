mod memory;
pub mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemorySecrets;

#[cfg(feature = "postgres")]
pub use postgres::PostgresSecrets;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteSecrets;

#[cfg(test)]
mod memory_tests {
    use async_trait::async_trait;

    use crate::InMemorySecrets;
    use crate::suite::Workbench;

    struct InMemory(InMemorySecrets);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemorySecrets;

        async fn setup() -> Self {
            Self(InMemorySecrets::default())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
