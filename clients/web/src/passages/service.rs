use gloo_net::http::Request;
use passages_contract::PassageDTO;

use crate::http::{ApiError, parsed};
use crate::passages::model::PassageId;

const PASSAGES: &str = "/api/passages";
const SUBJECT: &str = "passage";

pub async fn open(id: &PassageId) -> Result<PassageId, ApiError> {
    let response = Request::get(&format!("{PASSAGES}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let found: PassageDTO = parsed(response, SUBJECT).await?;

    Ok(PassageId::from(found.id))
}
