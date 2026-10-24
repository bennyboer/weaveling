use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Agent;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Act {
    Read,
    Write,
    Manage,
}

#[derive(Debug, Error)]
pub enum AuthorizerError {
    #[error("who may do what could not be asked")]
    Unreachable(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait Authorizer: Send + Sync {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError>;
}

#[derive(Debug, Default)]
pub struct AllowingEverything;

#[async_trait]
impl Authorizer for AllowingEverything {
    async fn may(
        &self,
        _agent: &Agent,
        _act: Act,
        _project: &str,
    ) -> Result<bool, AuthorizerError> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn where_one_person_does_the_work_they_may_do_everything() {
        for agent in [
            Agent::Local,
            Agent::System,
            Agent::Account("account_ada".into()),
        ] {
            for act in [Act::Read, Act::Write, Act::Manage] {
                assert!(
                    AllowingEverything
                        .may(&agent, act, "project_1")
                        .await
                        .expect("nothing to ask")
                );
            }
        }
    }
}
