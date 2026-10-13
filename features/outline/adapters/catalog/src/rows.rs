use outline_core::{Attachment, CatalogError};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("the catalog holds an attachment of a kind it does not know: {0}")]
struct UnknownKind(String);

pub fn attachment_type_of(attachment: &Attachment) -> &'static str {
    match attachment {
        Attachment::Scene(_) => "scene",
        Attachment::Idea(_) => "idea",
    }
}

pub fn attachment_from(attachment_type: &str, id: &str) -> Result<Attachment, CatalogError> {
    match attachment_type {
        "scene" => Ok(Attachment::scene(id)),
        "idea" => Ok(Attachment::idea(id)),
        unknown => Err(CatalogError::Backend(Box::new(UnknownKind(
            unknown.to_owned(),
        )))),
    }
}
