use eventsourcing::{Agent, AggregateId, ServiceError};

use crate::error::ReadingsError;
use crate::id::ReadingCopyId;
use crate::reading_copy::{ReadingCopyCommand, ReadingCopyError};
use crate::readings::ReadingsCore;

impl ReadingsCore {
    /// Deletes, as the system, at most so many copies whose deletion has come, with what they hold.
    /// Returns how many it deleted; a copy deleted already, and not yet gone from the list, is not counted.
    pub async fn delete_due_copies(&self, at_most: u64) -> Result<u64, ReadingsError> {
        let mut deleted = 0;
        for copy in self
            .copies
            .due_for_deletion(self.clock.now(), at_most)
            .await?
        {
            if self.deleted_by_the_system(&copy).await? {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    /// Deletes a copy as the system, with what it holds; whether it was there to delete.
    async fn deleted_by_the_system(&self, copy: &ReadingCopyId) -> Result<bool, ReadingsError> {
        let deleted = transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .execute(
                    transaction,
                    &AggregateId::from(copy),
                    ReadingCopyCommand::Delete,
                    &Agent::System,
                )
                .await?;
            self.reading_copy_contents.forget(transaction, copy).await?;

            Ok::<_, ReadingsError>(())
        });

        match deleted {
            Ok(()) => Ok(true),
            Err(ReadingsError::Events(ServiceError::Refused(ReadingCopyError::Deleted)))
            | Err(ReadingsError::Events(ServiceError::NotFound { .. })) => Ok(false),
            Err(failed) => Err(failed),
        }
    }
}
