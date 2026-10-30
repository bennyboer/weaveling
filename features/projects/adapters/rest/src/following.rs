use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use eventsourcing::Agent;
use live::{About, AccessWatch, Broadcaster, Gate, GatedTopic, Refused, Sieve, Subscribed, Topic};
use live_contract::FrameDTO;
use projects_core::ProjectsCore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectChanges {
    Listed,
    Of(String),
}

impl Display for ProjectChanges {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listed => f.write_str("every project"),
            Self::Of(project) => f.write_str(project),
        }
    }
}

pub struct ProjectsTopic {
    projects: ProjectsCore,
    broadcaster: Broadcaster<ProjectChanges>,
    one_project: GatedTopic<ProjectChanges>,
}

impl ProjectsTopic {
    pub fn new(projects: ProjectsCore, broadcaster: Broadcaster<ProjectChanges>) -> Self {
        Self {
            one_project: GatedTopic::new(
                "projects",
                broadcaster.clone(),
                |about| about.get("project").cloned().map(ProjectChanges::Of),
                Arc::new(ProjectsGate(projects.clone())),
            ),
            projects,
            broadcaster,
        }
    }
}

#[async_trait]
impl Topic for ProjectsTopic {
    fn named(&self) -> &'static str {
        "projects"
    }

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        if about.contains_key("project") {
            return self.one_project.subscribe(agent, about).await;
        }
        if about.get("mine").map(String::as_str) != Some("true") {
            return Err(Refused::NotThere);
        }

        Ok(
            Subscribed::hearing(self.broadcaster.hear(ProjectChanges::Listed)).sifted(Arc::new(
                ReadableBy {
                    agent: agent.clone(),
                    projects: self.projects.clone(),
                },
            )),
        )
    }
}

struct ProjectsGate(ProjectsCore);

#[async_trait]
impl Gate for ProjectsGate {
    async fn admits(
        &self,
        agent: &Agent,
        project: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(self.0.may_follow(project, agent).await?)
    }

    async fn watch(&self, project: &str) -> AccessWatch {
        self.0.watch_access(project)
    }
}

struct ReadableBy {
    agent: Agent,
    projects: ProjectsCore,
}

#[async_trait]
impl Sieve for ReadableBy {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let Some(project) = frame.of.as_deref() else {
            return Ok(false);
        };

        Ok(self.projects.may_follow(project, &self.agent).await?)
    }
}
