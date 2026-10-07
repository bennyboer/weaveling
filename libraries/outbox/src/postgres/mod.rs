mod enqueuing;
mod outbox;
mod schema;

#[cfg(test)]
mod tests;

pub use enqueuing::enqueue;
pub use outbox::PostgresOutbox;
pub use schema::migrations;

fn as_bigint(version: u64) -> i64 {
    i64::try_from(version)
        .expect("no stream reaches nine quintillion events, so a version past i64::MAX is a bug")
}
