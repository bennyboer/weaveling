use gloo_net::http::Request;
use messaging_contract::RefusalDTO;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http;
use crate::http::ApiError;
use crate::refusals::model::Refusal;

const REFUSALS: &str = "/api/service/refusals";
const SUBJECT: &str = "refusal";

pub async fn all() -> Result<Vec<Refusal>, ApiError> {
    let response = http::sent(Request::get(REFUSALS)).await?;
    let refused: Vec<RefusalDTO> = http::parsed(response, SUBJECT).await?;

    refused.into_iter().map(to_refusal).collect()
}

pub async fn retry(refusal: i64) -> Result<(), ApiError> {
    act_on(refusal, "retry").await
}

pub async fn acknowledge(refusal: i64) -> Result<(), ApiError> {
    act_on(refusal, "acknowledge").await
}

async fn act_on(refusal: i64, deed: &str) -> Result<(), ApiError> {
    let response = http::sent(Request::post(&format!("{REFUSALS}/{refusal}/{deed}"))).await?;
    http::checked(response, SUBJECT).await?;

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
