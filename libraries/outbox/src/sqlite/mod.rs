mod enqueuing;
pub mod instant;
mod outbox;
mod schema;

#[cfg(test)]
mod outbox_tests;
#[cfg(test)]
mod tests;

pub use enqueuing::enqueue;
pub use outbox::SqliteOutbox;
pub use schema::migrations;

fn as_integer(version: u64) -> i64 {
    i64::try_from(version)
        .expect("no stream reaches nine quintillion events, so a version past i64::MAX is a bug")
}
