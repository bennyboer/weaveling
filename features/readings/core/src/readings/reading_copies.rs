use authorizing::Act;
use eventsourcing::{AccountId, Agent, AggregateId, ServiceError};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::catalog::ReadingCopySummary;
use crate::contents::Contents;
use crate::cutting;
use crate::error::ReadingsError;
use crate::id::ReadingCopyId;
use crate::link::{ProjectLink, ReadingCopyLink};
use crate::reading_copies::{ListedReadingCopy, Shared};
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

        let mut listed = self
            .listed(self.copies.copies_of(&ProjectLink::from(project)).await?)
            .await?;
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
        let (contents, extent) = cutting::cut(self.work_as_it_is(project.clone()).await?, shared)
            .map_err(|_| ReadingsError::OutsideTheWork)?;
        let times = Times {
            open_until,
            deleted_on,
        };

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
            self.reading_copy_contents
                .keep(transaction, &copy, &contents)
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
    pub async fn reading_copy(
        &self,
        copy: &str,
        agent: &Agent,
    ) -> Result<(ListedReadingCopy, Contents), ReadingsError> {
        let (id, listed) = self.readable_copy(copy, agent).await?;
        let contents = self
            .reading_copy_contents
            .contents_of(&id)
            .await?
            .unwrap_or_else(|| Contents {
                project: listed.project.clone(),
                title: String::new(),
                parts: vec![],
            });

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

    /// Deletes a reading copy for good, with what it holds, for whoever made it or an owner of its project.
    pub async fn delete_copy(&self, copy: &str, agent: &Agent) -> Result<(), ReadingsError> {
        // TODO: its feedback deleted with it (slice 2)
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
            let standing = self.standing_within(transaction, &id).await?;
            if standing.is_none() {
                self.reading_copy_contents.forget(transaction, &id).await?;
            }

            Ok(standing)
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
        let mut listed = self.listed(vec![summary]).await?;
        self.mark_who_may_change(&mut listed, agent).await?;

        Ok(listed.remove(0))
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

    /// Copies as their project's members find them listed, each named for whoever made it and open until its opening has passed.
    async fn listed(
        &self,
        summaries: Vec<ReadingCopySummary>,
    ) -> Result<Vec<ListedReadingCopy>, ReadingsError> {
        let makers: Vec<AccountId> = summaries
            .iter()
            .filter_map(|summary| match &summary.made_by {
                Agent::Account(account) => Some(account.clone()),
                _ => None,
            })
            .collect();
        let names = self.names.names_of(&makers).await?;
        let now = self.clock.now();

        Ok(summaries
            .into_iter()
            .map(|summary| ListedReadingCopy {
                copy: ReadingCopyLink::from(summary.copy.to_string()),
                project: summary.project,
                name: summary.name.to_string(),
                maker: match &summary.made_by {
                    Agent::Account(account) => names.get(account).cloned(),
                    _ => None,
                },
                made_by: summary.made_by,
                made_at: summary.made_at,
                extent: summary.extent,
                open_until: summary.times.open_until,
                deleted_on: summary.times.deleted_on,
                open: summary.times.open_until.is_none_or(|until| until > now),
                may_change: false,
            })
            .collect())
    }
}
