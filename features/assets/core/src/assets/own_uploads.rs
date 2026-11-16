use std::collections::HashMap;

use authorizing::Act;
use eventsourcing::Agent;

use crate::asset::{AssetType, Owner, ProjectLink};
use crate::assets::AssetsCore;
use crate::catalog::{UploadMark, UploadsOf, UploadsSorted};
use crate::error::AssetsError;
use crate::id::AssetId;
use crate::storage::{Listed, OwnUpload};
use crate::uses::AssetUse;

/// How many bytes an account's uploads use together, and their limit.
/// The limit is missing if there is none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnUsage {
    pub used: u64,
    pub most: Option<u64>,
}

impl AssetsCore {
    /// Lists everything the signed-in account uploaded, one page at a time.
    /// This includes uploads into projects and the account's own images.
    /// Each entry says whether the account may delete it.
    /// Given a type, it lists only the uploads of that type.
    /// Fails with `NoAccount` when nobody is signed in, which is always the case without accounts.
    pub async fn uploads_of(
        &self,
        sorted: UploadsSorted,
        of_type: Option<AssetType>,
        after: Option<&UploadMark>,
        at_most: u64,
        agent: &Agent,
    ) -> Result<Listed<OwnUpload, UploadMark>, AssetsError> {
        let Agent::Account(account) = agent else {
            return Err(AssetsError::NoAccount);
        };

        let (uploads, uses) = transacting::retrying!(&*self.transactions, |transaction| {
            let uploads = self
                .catalog
                .uploads_of(
                    transaction,
                    &UploadsOf::Account(account.clone()),
                    of_type,
                    sorted,
                    after,
                    at_most,
                )
                .await?;
            let ids: Vec<AssetId> = uploads.iter().map(|upload| upload.id).collect();
            let uses = self.uses.uses_of(transaction, &ids).await?;

            Ok::<_, AssetsError>((uploads, uses))
        })?;
        let mut writable: HashMap<ProjectLink, bool> = HashMap::new();
        let mut listed = Vec::with_capacity(uploads.len());
        for upload in uploads {
            let deletable = match &upload.owner {
                Owner::Account(owner) => owner == account,
                Owner::Project(project) => match writable.get(project) {
                    Some(may) => *may,
                    None => {
                        let may = self
                            .authorizer
                            .may(agent, Act::Write, project.as_str())
                            .await?;
                        writable.insert(project.clone(), may);
                        may
                    }
                },
            };
            let uses: Vec<AssetUse> = uses
                .iter()
                .filter(|(asset, _)| *asset == upload.id)
                .map(|(_, used)| used.clone())
                .collect();
            listed.push(OwnUpload {
                deletable: deletable && uses.is_empty(),
                upload,
                uses,
            });
        }
        let last = listed
            .last()
            .map(|own: &OwnUpload| UploadMark::from(&own.upload));

        Ok(Listed::paged(listed, last, at_most))
    }

    /// Returns how many bytes the signed-in account's uploads use, wherever they went, and its limit.
    /// The limit is the account's own, if an operator gave it one, or else the default for accounts.
    pub async fn usage_of(&self, agent: &Agent) -> Result<OwnUsage, AssetsError> {
        let Agent::Account(account) = agent else {
            return Err(AssetsError::NoAccount);
        };

        transacting::retrying!(&*self.transactions, |transaction| {
            let usage = self
                .catalog
                .usage_within(transaction, Some(account), None)
                .await?;
            let limits = self.limits_of(transaction, Some(account)).await?;

            Ok::<_, AssetsError>(OwnUsage {
                used: usage.account,
                most: limits.each_account,
            })
        })
    }
}
