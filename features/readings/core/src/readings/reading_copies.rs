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
        Ok(sample_copies(
            &ProjectLink::from(project),
            agent,
            self.clock.now(),
        ))
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
            &ProjectLink::from(SAMPLE_PROJECT),
            agent,
            now,
        ))
        .expect("there are two sample copies");
        made.name = name.to_owned();
        made.made_at = now;
        made.extent = match shared {
            Shared::Book => Extent::Book,
            Shared::Sections { .. } => Extent::Sections(1),
        };
        made.open_until = open_until;
        made.deleted_on = deleted_on;

        Ok(made)
    }

    /// One reading copy with what it holds, for whoever may read the project it was made of.
    /// To anyone else it is not there.
    pub async fn copy(
        &self,
        copy: &str,
        agent: &Agent,
    ) -> Result<(ListedReadingCopy, Contents), ReadingsError> {
        // TODO: the copy and its contents kept in the store, rather than the samples of one fixed project
        let listed = sample_copies(&ProjectLink::from(SAMPLE_PROJECT), agent, self.clock.now())
            .into_iter()
            .find(|listed| listed.copy.as_str() == copy)
            .ok_or(ReadingsError::ReadingCopyNotThere)?;
        if !self
            .authorizer
            .may(agent, Act::Read, listed.project.as_str())
            .await?
        {
            return Err(ReadingsError::ReadingCopyNotThere);
        }
        let contents = sample_contents(listed.project.clone());

        Ok((listed, contents))
    }
}

const SAMPLE_PROJECT: &str = "project_loom";

/// Two copies of any project: one the asking agent made of the whole book, still open, and an older one of a section by Hedy, closed.
fn sample_copies(
    project: &ProjectLink,
    agent: &Agent,
    now: OffsetDateTime,
) -> Vec<ListedReadingCopy> {
    vec![
        ListedReadingCopy {
            copy: ReadingCopyLink::from("copy_sample_book"),
            project: project.clone(),
            name: "Draft for Lena".to_owned(),
            made_by: agent.clone(),
            maker: "You".to_owned(),
            made_at: now - Duration::days(2),
            extent: Extent::Book,
            open_until: Some(now + Duration::days(28)),
            deleted_on: Some(now + Duration::days(88)),
            open: true,
        },
        ListedReadingCopy {
            copy: ReadingCopyLink::from("copy_sample_section"),
            project: project.clone(),
            name: "First draft, part one".to_owned(),
            made_by: Agent::Account(eventsourcing::AccountId::from("account_hedy")),
            maker: "Hedy".to_owned(),
            made_at: now - Duration::days(40),
            extent: Extent::Sections(1),
            open_until: Some(now - Duration::days(10)),
            deleted_on: None,
            open: false,
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
