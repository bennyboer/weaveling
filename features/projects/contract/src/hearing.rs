/// Lays out a feature's listener hearing which projects are in the trash, put there and restored, for its core to keep through `access::HearsAccess`. The feature's messaging crate depends on `access`, `async-trait`, `eventpublishing`, `eventsourcing`, `inbox`, `messaging` and `transacting`, which the listener is written in.
#[macro_export]
macro_rules! access_trash_listener {
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
            const LISTENS_TO: &'static [&'static str] = &[$crate::TRASHED, $crate::RESTORED];
            const WHEN_REFUSED: &'static str = $when_refused;

            async fn take_in(
                &self,
                transaction: &mut transacting::Transaction,
                message: &messaging::Message,
            ) -> Result<(), messaging::Failure> {
                let told = eventpublishing::published_in::<$crate::ProjectEventDTO>(message)?;
                let in_the_trash = match told.event.body {
                    $crate::ProjectEventDTO::Trashed { .. } => true,
                    $crate::ProjectEventDTO::Restored => false,
                    _ => return Err("this tells neither a trashing nor a restoring".into()),
                };

                Ok(access::HearsAccess::remember_trash(
                    &self.hearing,
                    Some(transaction),
                    &told.aggregate.id,
                    eventsourcing::Version::of(told.aggregate.version),
                    in_the_trash,
                )
                .await?)
            }
        }
    };
}
