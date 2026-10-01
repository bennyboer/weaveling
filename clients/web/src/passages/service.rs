use gloo_net::http::Request;
use passages_contract::{CreatePassageRequest, PassageDTO};

use crate::http::{ApiError, parsed};
use crate::passages::model::PassageId;
use crate::projects::model::ProjectId;

const PASSAGES: &str = "/api/passages";
const SUBJECT: &str = "passage";

pub async fn create(project: &ProjectId) -> Result<PassageId, ApiError> {
    let payload = CreatePassageRequest {
        project: project.to_string(),
    };
    let response = Request::post(PASSAGES)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let started: PassageDTO = parsed(response, SUBJECT).await?;

    Ok(PassageId::from(started.id))
}
