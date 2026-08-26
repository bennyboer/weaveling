use gloo_net::http::Request;
use passages_contract::PassageDTO;

use crate::http::{ApiError, checked, parsed};
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

pub async fn confirm(id: &PassageId) -> Result<(), ApiError> {
    let response = Request::get(&format!("{PASSAGES}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    checked(response, SUBJECT).await?;

    Ok(())
}
