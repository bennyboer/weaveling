use eventsourcing::{Agent, ServiceError};
use outline_core::{
    Attachment, CatalogError, OutlineCatalog, OutlineError, OutlineId, OutlineService,
    OutlineServiceError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotLetGo {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Outlines(#[from] OutlineServiceError),
}

pub async fn let_go_everywhere(
    outlines: &OutlineService,
    catalog: &dyn OutlineCatalog,
    gone: &Attachment,
) -> Result<(), NotLetGo> {
    catalog.mark_gone(gone).await?;

    for outline in catalog.outlines_holding(gone).await? {
        let_go(outlines, outline, gone).await?;
    }

    Ok(())
}

pub async fn let_go_of_what_is_gone(
    outlines: &OutlineService,
    catalog: &dyn OutlineCatalog,
    outline: OutlineId,
) -> Result<(), NotLetGo> {
    for gone in catalog.gone_but_held(outline).await? {
        let_go(outlines, outline, &gone).await?;
    }

    Ok(())
}

async fn let_go(
    outlines: &OutlineService,
    outline: OutlineId,
    gone: &Attachment,
) -> Result<(), NotLetGo> {
    match outlines
        .detach(&outline.to_string(), gone.clone(), None, &Agent::System)
        .await
    {
        Ok(_) => Ok(()),
        Err(refused) if nothing_left_to_detach(&refused) => Ok(()),
        Err(refused) => Err(refused.into()),
    }
}

fn nothing_left_to_detach(refused: &OutlineServiceError) -> bool {
    matches!(
        refused,
        OutlineServiceError::Events(ServiceError::Refused(
            OutlineError::NotAttached | OutlineError::Discarded
        ))
    )
}
