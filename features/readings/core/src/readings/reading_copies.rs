use authorizing::Act;
use eventsourcing::{Agent, AggregateId, ServiceError};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::catalog::ReadingCopySummary;
use crate::contents::{Block, Contents, Marks, Part, Run};
use crate::error::ReadingsError;
use crate::id::ReadingCopyId;
use crate::link::{ProjectLink, ReadingCopyLink, SceneLink, SectionLink};
use crate::reading_copies::{Extent, ListedReadingCopy, Shared};
use crate::reading_copy::{ReadingCopyCommand, Times};
use crate::reading_copy_name::ReadingCopyName;
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

        let mut listed: Vec<ListedReadingCopy> = self
            .copies
            .copies_of(&ProjectLink::from(project))
            .await?
            .into_iter()
            .map(|summary| self.listed(summary))
            .collect();
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
        let project = ProjectLink::from(project);
        let name = ReadingCopyName::new(name)?;
        let extent = self.extent_of(&project, shared).await?;
        let times = Times {
            open_until,
            deleted_on,
        };

        // TODO: what the copy holds, taken from the live work and kept apart under its id (step 10)
        let copy = ReadingCopyId::generate(self.clock.now());
        let made = transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .begin(
                    transaction,
                    &AggregateId::from(&copy),
                    ReadingCopyCommand::Make {
                        project: project.clone(),
                        name: name.clone(),
                        extent,
                        times,
                    },
                    agent,
                )
                .await?;
            self.standing_within(transaction, &copy).await
        })?;

        self.listed_for(made.ok_or(ReadingsError::ReadingCopyNotThere)?, agent)
            .await
    }

    /// Keeps a copy in its project's list as it now stands, or forgets it once it is deleted.
    /// It reads the copy's latest state, so a message that arrives late or twice changes nothing.
    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        copy: &ReadingCopyId,
    ) -> Result<(), ReadingsError> {
        let standing = self.events.latest(&AggregateId::from(copy)).await?;

        transacting::joining!(&*self.transactions, within, |transaction| {
            if standing.state.is_deleted() {
                self.copies.forget(transaction, copy).await?;
            } else {
                self.copies
                    .remember(transaction, &ReadingCopySummary::of(*copy, &standing.state))
                    .await?;
            }

            Ok(())
        })
    }

    /// One reading copy with what it holds, for whoever may read the project it was made of.
    /// To anyone else it is not there.
    pub async fn copy(
        &self,
        copy: &str,
        agent: &Agent,
    ) -> Result<(ListedReadingCopy, Contents), ReadingsError> {
        let (_, listed) = self.readable_copy(copy, agent).await?;
        // TODO: what the copy holds, as it was kept when it was made (step 10)
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
        let name = ReadingCopyName::new(name)?;

        self.changed_and_listed(copy, ReadingCopyCommand::Rename(name), agent)
            .await
    }

    /// Moves a reading copy's times later, for whoever made it or an owner of its project.
    /// Times earlier than they were, and an opening beyond the deletion, are refused.
    pub async fn extend_copy(
        &self,
        copy: &str,
        open_until: Option<OffsetDateTime>,
        deleted_on: Option<OffsetDateTime>,
        agent: &Agent,
    ) -> Result<ListedReadingCopy, ReadingsError> {
        self.changed_and_listed(
            copy,
            ReadingCopyCommand::Extend(Times {
                open_until,
                deleted_on,
            }),
            agent,
        )
        .await
    }

    /// Deletes a reading copy for good, for whoever made it or an owner of its project.
    pub async fn delete_copy(&self, copy: &str, agent: &Agent) -> Result<(), ReadingsError> {
        // TODO: its contents and feedback deleted with it (steps 10 and 11)
        self.changed(copy, ReadingCopyCommand::Delete, agent)
            .await?;

        Ok(())
    }

    /// Changes a copy the agent may change, and answers it as it is listed now.
    async fn changed_and_listed(
        &self,
        copy: &str,
        command: ReadingCopyCommand,
        agent: &Agent,
    ) -> Result<ListedReadingCopy, ReadingsError> {
        let changed = self.changed(copy, command, agent).await?;

        self.listed_for(changed.ok_or(ReadingsError::ReadingCopyNotThere)?, agent)
            .await
    }

    /// Changes a copy the agent may change, and answers it as it now stands; one the change deleted is none.
    async fn changed(
        &self,
        copy: &str,
        command: ReadingCopyCommand,
        agent: &Agent,
    ) -> Result<Option<ReadingCopySummary>, ReadingsError> {
        let id = self.changeable_copy(copy, agent).await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .execute(transaction, &AggregateId::from(&id), command.clone(), agent)
                .await?;
            self.standing_within(transaction, &id).await
        })
    }

    /// The copy as it stands in the transaction that changed it; once deleted, none.
    async fn standing_within(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyId,
    ) -> Result<Option<ReadingCopySummary>, ReadingsError> {
        let standing = self
            .events
            .latest_within(transaction, &AggregateId::from(copy))
            .await?;

        Ok((!standing.state.is_deleted()).then(|| ReadingCopySummary::of(*copy, &standing.state)))
    }

    /// The copy as listed for an agent who may read its project.
    /// To anyone else, and once it is deleted, it is not there.
    async fn readable_copy(
        &self,
        copy: &str,
        agent: &Agent,
    ) -> Result<(ReadingCopyId, ListedReadingCopy), ReadingsError> {
        let id = copy
            .parse::<ReadingCopyId>()
            .map_err(|_| ReadingsError::ReadingCopyNotThere)?;
        let standing = match self.events.latest(&AggregateId::from(&id)).await {
            Ok(standing) => standing.state,
            Err(ServiceError::NotFound { .. }) => return Err(ReadingsError::ReadingCopyNotThere),
            Err(failed) => return Err(failed.into()),
        };
        if standing.is_deleted()
            || !self
                .authorizer
                .may(agent, Act::Read, standing.project().as_str())
                .await?
        {
            return Err(ReadingsError::ReadingCopyNotThere);
        }
        let listed = self
            .listed_for(ReadingCopySummary::of(id, &standing), agent)
            .await?;

        Ok((id, listed))
    }

    /// The copy, for an agent who may change it; a member who may not is refused.
    async fn changeable_copy(
        &self,
        copy: &str,
        agent: &Agent,
    ) -> Result<ReadingCopyId, ReadingsError> {
        let (id, listed) = self.readable_copy(copy, agent).await?;
        if !listed.may_change {
            return Err(ReadingsError::NotYours);
        }

        Ok(id)
    }

    async fn listed_for(
        &self,
        summary: ReadingCopySummary,
        agent: &Agent,
    ) -> Result<ListedReadingCopy, ReadingsError> {
        let mut listed = [self.listed(summary)];
        self.mark_who_may_change(&mut listed, agent).await?;
        let [listed] = listed;

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

    /// A copy as its project's members find it listed, open until its opening has passed.
    fn listed(&self, summary: ReadingCopySummary) -> ListedReadingCopy {
        ListedReadingCopy {
            copy: ReadingCopyLink::from(summary.copy.to_string()),
            project: summary.project,
            name: summary.name.to_string(),
            made_by: summary.made_by,
            // TODO: the maker's name as the project's members know it (9b3)
            maker: String::new(),
            made_at: summary.made_at,
            extent: summary.extent,
            open_until: summary.times.open_until,
            deleted_on: summary.times.deleted_on,
            open: summary
                .times
                .open_until
                .is_none_or(|until| until > self.clock.now()),
            may_change: false,
        }
    }

    /// How many sections or scenes of the work as it is now a copy is to hold, counted in reading order from the first to the last.
    async fn extent_of(
        &self,
        project: &ProjectLink,
        shared: &Shared,
    ) -> Result<Extent, ReadingsError> {
        let (first, last) = match shared {
            Shared::Book => return Ok(Extent::Book),
            Shared::Sections { first, last } => (first.as_str(), last.as_str()),
            Shared::Scenes { first, last } => (first.as_str(), last.as_str()),
        };
        let Some(outline) = self.outlines.outline_of(project).await? else {
            return Err(ReadingsError::ProjectNotThere);
        };
        let in_order: Vec<&str> = match shared {
            Shared::Sections { .. } => outline
                .sections
                .iter()
                .map(|section| section.section.as_str())
                .collect(),
            _ => outline
                .sections
                .iter()
                .flat_map(|section| section.scenes.iter().map(SceneLink::as_str))
                .collect(),
        };
        let from = in_order.iter().position(|one| *one == first);
        let to = in_order.iter().position(|one| *one == last);
        let count = match (from, to) {
            (Some(from), Some(to)) if from <= to => {
                u32::try_from(to - from + 1).map_err(|_| ReadingsError::OutsideTheWork)?
            }
            _ => return Err(ReadingsError::OutsideTheWork),
        };

        Ok(match shared {
            Shared::Sections { .. } => Extent::Sections(count),
            _ => Extent::Scenes(count),
        })
    }
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
