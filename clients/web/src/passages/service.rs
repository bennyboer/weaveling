use gloo_net::http::Request;
use passages_contract::PassageDTO;

use crate::http::{ApiError, parsed};
use crate::passages::model::PassageId;

const PASSAGES: &str = "/api/passages";
const SUBJECT: &str = "passage";

pub async fn create() -> Result<PassageId, ApiError> {
    let response = Request::post(PASSAGES)
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let started: PassageDTO = parsed(response, SUBJECT).await?;

    Ok(PassageId::from(started.id))
}
