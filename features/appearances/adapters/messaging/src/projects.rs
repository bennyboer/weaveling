use appearances_core::{AppearanceCatalog, CatalogError, ProjectLink, Subject};
use messaging::Message;
use projects_contract::context::PROJECT;

pub(crate) async fn remember_project_of(
    catalog: &dyn AppearanceCatalog,
    subject: &Subject,
    message: &Message,
) -> Result<(), CatalogError> {
    match eventpublishing::context_in(message).get(PROJECT) {
        Some(project) => {
            catalog
                .remember_project(subject, &ProjectLink::from(project))
                .await
        }
        None => Ok(()),
    }
}
