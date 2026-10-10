use gloo_net::http::Request;
use messaging_contract::RefusalDTO;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http::{ApiError, checked, parsed};
use crate::refusals::model::Refusal;

const REFUSALS: &str = "/api/service/refusals";
const SUBJECT: &str = "refusal";

pub async fn all() -> Result<Vec<Refusal>, ApiError> {
    let response = Request::get(REFUSALS)
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let refused: Vec<RefusalDTO> = parsed(response, SUBJECT).await?;

    refused.into_iter().map(to_refusal).collect()
}

pub async fn retry(refusal: i64) -> Result<(), ApiError> {
    act_on(refusal, "retry").await
}

pub async fn acknowledge(refusal: i64) -> Result<(), ApiError> {
    act_on(refusal, "acknowledge").await
}

async fn act_on(refusal: i64, deed: &str) -> Result<(), ApiError> {
    let response = Request::post(&format!("{REFUSALS}/{refusal}/{deed}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    checked(response, SUBJECT).await?;

    Ok(())
}

fn to_refusal(dto: RefusalDTO) -> Result<Refusal, ApiError> {
    Ok(Refusal {
        id: dto.id,
        listener: dto.listener,
        routing: dto.routing,
        attempts: dto.attempts,
        why: dto.why,
        plainly: dto.plainly,
        given_up_at: OffsetDateTime::parse(&dto.given_up_at, &Rfc3339)
            .map_err(|_| ApiError::Unexpected)?,
        acknowledged: dto.acknowledged_at.is_some(),
    })
}
