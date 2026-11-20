/// Defines a feature's listener that hears which projects stand: started, put in the trash, restored, and deleted.
/// The listener passes this to the feature's core through `access::HearsAccess`:
/// a project started as out of the trash, so the feature has heard of it, and a project deleted as disbanded.
/// Where there are no accounts, this is all that says whether a project may be worked on.
///
/// The feature's messaging crate must depend on `access`, `async-trait`, `eventpublishing`, `eventsourcing`, `inbox`, `messaging` and `transacting`, because the listener's code uses them.
#[macro_export]
macro_rules! access_standing_listener {
    ($listener:ident, $core:ty, $name:literal, $when_refused:literal) => {
        pub struct $listener {
            hearing: $core,
        }

        impl $listener {
            pub fn new(hearing: $core) -> Self {
                Self { hearing }
            }
        }

        #[async_trait::async_trait]
        impl inbox::KeptHandler for $listener {
            const NAME: &'static str = $name;
            const LISTENS_TO: &'static [&'static str] = &[
                $crate::STARTED,
                $crate::TRASHED,
                $crate::RESTORED,
                $crate::DELETED,
            ];
            const WHEN_REFUSED: &'static str = $when_refused;

            async fn take_in(
                &self,
                transaction: &mut transacting::Transaction,
                message: &messaging::Message,
            ) -> Result<(), messaging::Failure> {
                let told = eventpublishing::published_in::<$crate::ProjectEventDTO>(message)?;
                let project = &told.aggregate.id;
                let version = eventsourcing::Version::of(told.aggregate.version);
                let in_the_trash = match told.event.body {
                    $crate::ProjectEventDTO::Started { .. } | $crate::ProjectEventDTO::Restored => {
                        false
                    }
                    $crate::ProjectEventDTO::Trashed { .. } => true,
                    $crate::ProjectEventDTO::Deleted => {
                        return Ok(access::HearsAccess::remember_disbanded(
                            &self.hearing,
                            Some(transaction),
                            project,
                            version,
                        )
                        .await?);
                    }
                    _ => {
                        return Err(
                            "this tells neither a starting, a trashing, a restoring nor a deleting"
                                .into(),
                        );
                    }
                };

                Ok(access::HearsAccess::remember_trash(
                    &self.hearing,
                    Some(transaction),
                    project,
                    version,
                    in_the_trash,
                )
                .await?)
            }
        }
    };
}
