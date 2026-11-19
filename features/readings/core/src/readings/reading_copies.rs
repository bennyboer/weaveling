use authorizing::Act;
use eventsourcing::Agent;
use time::{Duration, OffsetDateTime};

use crate::contents::{Block, Contents, Marks, Part, Run};
use crate::error::ReadingsError;
use crate::link::{ProjectLink, ReadingCopyLink, SceneLink, SectionLink};
use crate::reading_copies::{Extent, ListedReadingCopy, Shared};
use crate::readings::ReadingsCore;

impl ReadingsCore {
    /// Every reading copy of a project, newest first, for whoever may read the project.
    pub async fn copies_of(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<Vec<ListedReadingCopy>, ReadingsError> {
        if !self.authorizer.may(agent, Act::Read, project).await? {
            return Err(ReadingsError::ProjectNotThere);
        }

        // TODO: the copies kept in the store, rather than the same sample for every project
        let mut listed = sample_copies(&ProjectLink::from(project), agent, self.clock.now());
        self.mark_who_may_change(&mut listed, agent).await?;

        Ok(listed)
    }

    /// Makes a reading copy of the work as it is now, for any member of the project.
    /// Readers make copies too, as commenting on the live work makes one; only sharing a copy by a link is for authors.
    pub async fn make_copy(
        &self,
        project: &str,
        name: &str,
        shared: &Shared,
        open_until: Option<OffsetDateTime>,
        deleted_on: Option<OffsetDateTime>,
        agent: &Agent,
    ) -> Result<ListedReadingCopy, ReadingsError> {
        if !self.authorizer.may(agent, Act::Read, project).await? {
            return Err(ReadingsError::ProjectNotThere);
        }

        // TODO: the copy made from the live work and kept, by the ReadingCopy aggregate's rules
        let now = self.clock.now();
        let [mut made, ..] = <[ListedReadingCopy; 2]>::try_from(sample_copies(
            &ProjectLink::from(project),
            agent,
            now,
        ))
        .expect("there are two sample copies");
        made.name = name.to_owned();
        made.made_at = now;
        made.extent = match shared {
            Shared::Book => Extent::Book,
            Shared::Sections { .. } => Extent::Sections(1),
            Shared::Scenes { .. } => Extent::Scenes(1),
        };
        made.open_until = open_until;
        made.deleted_on = deleted_on;
        made.may_change = true;

        Ok(made)
    }

    /// One reading copy with what it holds, for whoever may read the project it was made of.
    /// To anyone else it is not there.
    pub async fn copy(
        &self,
        copy: &str,
        agent: &Agent,
    ) -> Result<(ListedReadingCopy, Contents), ReadingsError> {
        let listed = self.readable_copy(copy, agent).await?;
        let contents = sample_contents(listed.project.clone());

        Ok((listed, contents))
    }

    /// Renames a reading copy, for whoever made it or an owner of its project.
    pub async fn rename_copy(
        &self,
        copy: &str,
        name: &str,
        agent: &Agent,
    ) -> Result<ListedReadingCopy, ReadingsError> {
        let mut renamed = self.changeable_copy(copy, agent).await?;

        // TODO: renamed by the ReadingCopy aggregate and kept
        renamed.name = name.to_owned();

        Ok(renamed)
    }

    /// Moves a reading copy's times later, for whoever made it or an owner of its project.
    pub async fn extend_copy(
        &self,
        copy: &str,
        open_until: Option<OffsetDateTime>,
        deleted_on: Option<OffsetDateTime>,
        agent: &Agent,
    ) -> Result<ListedReadingCopy, ReadingsError> {
        let mut extended = self.changeable_copy(copy, agent).await?;

        // TODO: extended by the ReadingCopy aggregate and kept, which refuses times earlier than they were, and an opening beyond the deletion
        extended.open_until = open_until;
        extended.deleted_on = deleted_on;
        extended.open = open_until.is_none_or(|until| until > self.clock.now());

        Ok(extended)
    }

    /// Deletes a reading copy for good, with its feedback, for whoever made it or an owner of its project.
    pub async fn delete_copy(&self, copy: &str, agent: &Agent) -> Result<(), ReadingsError> {
        self.changeable_copy(copy, agent).await?;

        // TODO: deleted by the ReadingCopy aggregate, its contents and feedback with it
        Ok(())
    }

    /// The copy as listed for an agent who may read its project; to anyone else it is not there.
    async fn readable_copy(
        &self,
        copy: &str,
        agent: &Agent,
    ) -> Result<ListedReadingCopy, ReadingsError> {
        // TODO: the copy kept in the store, rather than samples that carry their project in their id
        let project = sample_project_of(copy).ok_or(ReadingsError::ReadingCopyNotThere)?;
        let mut listed: Vec<ListedReadingCopy> = sample_copies(&project, agent, self.clock.now())
            .into_iter()
            .filter(|listed| listed.copy.as_str() == copy)
            .collect();
        if listed.is_empty()
            || !self
                .authorizer
                .may(agent, Act::Read, project.as_str())
                .await?
        {
            return Err(ReadingsError::ReadingCopyNotThere);
        }
        self.mark_who_may_change(&mut listed, agent).await?;

        Ok(listed.remove(0))
    }

    /// The copy for an agent who may change it; a member who may not is refused.
    async fn changeable_copy(
        &self,
        copy: &str,
        agent: &Agent,
    ) -> Result<ListedReadingCopy, ReadingsError> {
        let listed = self.readable_copy(copy, agent).await?;
        if !listed.may_change {
            return Err(ReadingsError::NotYours);
        }

        Ok(listed)
    }

    /// Whoever made a copy may change it, and an owner of its project may change every copy of it.
    async fn mark_who_may_change(
        &self,
        listed: &mut [ListedReadingCopy],
        agent: &Agent,
    ) -> Result<(), ReadingsError> {
        let Some(project) = listed.first().map(|copy| copy.project.clone()) else {
            return Ok(());
        };
        let owner = self
            .authorizer
            .may(agent, Act::Manage, project.as_str())
            .await?;
        for copy in listed {
            copy.may_change = owner || &copy.made_by == agent;
        }

        Ok(())
    }
}

/// The project a sample copy's id names, as `copy--{project}--{what}`.
fn sample_project_of(copy: &str) -> Option<ProjectLink> {
    let (project, _) = copy.strip_prefix("copy--")?.rsplit_once("--")?;

    Some(ProjectLink::from(project))
}

fn sample_id(project: &ProjectLink, what: &str) -> ReadingCopyLink {
    ReadingCopyLink::from(format!("copy--{project}--{what}"))
}

/// Two copies of any project: one the asking agent made of the whole book, still open, and an older one of a section by Hedy, closed.
fn sample_copies(
    project: &ProjectLink,
    agent: &Agent,
    now: OffsetDateTime,
) -> Vec<ListedReadingCopy> {
    vec![
        ListedReadingCopy {
            copy: sample_id(project, "book"),
            project: project.clone(),
            name: "Draft for Lena".to_owned(),
            made_by: agent.clone(),
            maker: "You".to_owned(),
            made_at: now - Duration::days(2),
            extent: Extent::Book,
            open_until: Some(now + Duration::days(28)),
            deleted_on: Some(now + Duration::days(88)),
            open: true,
            may_change: false,
        },
        ListedReadingCopy {
            copy: sample_id(project, "section"),
            project: project.clone(),
            name: "First draft, part one".to_owned(),
            made_by: Agent::Account(eventsourcing::AccountId::from("account_hedy")),
            maker: "Hedy".to_owned(),
            made_at: now - Duration::days(40),
            extent: Extent::Sections(1),
            open_until: Some(now - Duration::days(10)),
            deleted_on: None,
            open: false,
            may_change: false,
        },
    ]
}

fn sample_contents(project: ProjectLink) -> Contents {
    Contents {
        project,
        title: "The Loom".to_owned(),
        parts: vec![
            Part::Section {
                section: SectionLink::from("section_morning"),
                depth: 0,
                title: "Morning".to_owned(),
            },
            Part::Scene {
                scene: SceneLink::from("scene_harbour"),
                blocks: vec![Block::Paragraph(vec![
                    Run::plain("The boats came in before the light did, "),
                    Run::Text {
                        text: "one by one".to_owned(),
                        marks: Marks {
                            emphasis: true,
                            ..Marks::default()
                        },
                    },
                    Run::plain("."),
                ])],
            },
            Part::Scene {
                scene: SceneLink::from("scene_wall"),
                blocks: vec![Block::Paragraph(vec![Run::plain(
                    "Hedy counted them from the wall.",
                )])],
            },
        ],
    }
}
