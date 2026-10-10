use gloo_net::http::Request;
use messaging_contract::RefusalDTO;

use crate::http::{ApiError, parsed};
use crate::refusals::model::Refusal;

const REFUSALS: &str = "/api/service/refusals";
const SUBJECT: &str = "refusals";

pub async fn all() -> Result<Vec<Refusal>, ApiError> {
    let response = Request::get(REFUSALS)
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let refused: Vec<RefusalDTO> = parsed(response, SUBJECT).await?;

    Ok(refused.into_iter().map(to_refusal).collect())
}

fn to_refusal(dto: RefusalDTO) -> Refusal {
    Refusal {
        id: dto.id,
        acknowledged: dto.acknowledged_at.is_some(),
    }
}
