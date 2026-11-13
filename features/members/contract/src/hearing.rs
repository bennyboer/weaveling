/// Defines a feature's listener that hears who is a member of which project, in which role.
/// The listener passes this to the feature's core through `access::HearsAccess`.
/// It handles these events: founded, joined, role changed, left, removed and disbanded.
/// An abandoning, which follows the removal of the last member, changes nobody's membership.
///
/// The feature's messaging crate must depend on `access`, `async-trait`, `eventpublishing`, `eventsourcing`, `inbox`, `messaging` and `transacting`, because the listener's code uses them.
#[macro_export]
macro_rules! access_roles_listener {
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
                $crate::FOUNDED,
                $crate::JOINED,
                $crate::ROLE_CHANGED,
                $crate::LEFT,
                $crate::REMOVED,
                $crate::DISBANDED,
            ];
            const WHEN_REFUSED: &'static str = $when_refused;

            async fn take_in(
                &self,
                transaction: &mut transacting::Transaction,
                message: &messaging::Message,
            ) -> Result<(), messaging::Failure> {
                let role_named = |named: &str| -> Result<access::Role, messaging::Failure> {
                    match named {
                        $crate::role::OWNER => Ok(access::Role::Owner),
                        $crate::role::AUTHOR => Ok(access::Role::Author),
                        $crate::role::READER => Ok(access::Role::Reader),
                        _ => Err("the members name a role there is none of".into()),
                    }
                };
                let told = eventpublishing::published_in::<$crate::MembersEventDTO>(message)?;
                let project = told.aggregate.id;
                let version = eventsourcing::Version::of(told.aggregate.version);
                let (account, role) = match told.event.body {
                    $crate::MembersEventDTO::Founded { owner } => {
                        (owner, Some(access::Role::Owner))
                    }
                    $crate::MembersEventDTO::Joined { account, role } => {
                        (account, Some(role_named(&role)?))
                    }
                    $crate::MembersEventDTO::RoleChanged { account, to } => {
                        (account, Some(role_named(&to)?))
                    }
                    $crate::MembersEventDTO::Left { account }
                    | $crate::MembersEventDTO::Removed { account } => (account, None),
                    $crate::MembersEventDTO::Abandoned => return Ok(()),
                    $crate::MembersEventDTO::Disbanded => {
                        return Ok(access::HearsAccess::remember_disbanded(
                            &self.hearing,
                            Some(transaction),
                            &project,
                            version,
                        )
                        .await?);
                    }
                };

                Ok(access::HearsAccess::remember_role(
                    &self.hearing,
                    Some(transaction),
                    &project,
                    &eventsourcing::AccountId::from(account),
                    version,
                    role,
                )
                .await?)
            }
        }
    };
}
