/// Defines a feature's listener for the events that change who runs the instance.
///
/// The listener hears when an account becomes an operator, stops being one, or is deleted.
/// It passes each change to the feature's core through `access::HearsOperators`.
/// The listener's code uses `access`, `async-trait`, `eventpublishing`, `eventsourcing`, `inbox`, `messaging` and `transacting`.
/// So the feature's messaging crate must depend on these crates.
#[macro_export]
macro_rules! access_operators_listener {
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
                $crate::BECAME_OPERATOR,
                $crate::STOPPED_BEING_OPERATOR,
                $crate::DELETED,
            ];
            const WHEN_REFUSED: &'static str = $when_refused;

            async fn take_in(
                &self,
                transaction: &mut transacting::Transaction,
                message: &messaging::Message,
            ) -> Result<(), messaging::Failure> {
                let told = eventpublishing::published_in::<$crate::AccountEventDTO>(message)?;
                let operator = match told.event.body {
                    $crate::AccountEventDTO::BecameOperator => true,
                    $crate::AccountEventDTO::StoppedBeingOperator
                    | $crate::AccountEventDTO::Deleted => false,
                    _ => return Err("this tells nothing of who runs the instance".into()),
                };

                Ok(access::HearsOperators::remember_operator(
                    &self.hearing,
                    Some(transaction),
                    &eventsourcing::AccountId::from(told.aggregate.id),
                    eventsourcing::Version::of(told.aggregate.version),
                    operator,
                )
                .await?)
            }
        }
    };
}
