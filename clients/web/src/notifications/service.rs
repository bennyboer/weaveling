use gloo_net::http::{Request, RequestBuilder};
use notifications_contract::{AboutDTO, NoticeDTO};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http;
use crate::http::ApiError;
use crate::members::listed::Role;
use crate::notifications::notice::RoleChange;

const NOTIFICATIONS: &str = "/api/notifications";
const NOTICE: &str = "notice";
const IF_MATCH: &str = "If-Match";

fn made_on(request: RequestBuilder, on: u64) -> RequestBuilder {
    match on {
        0 => request,
        on => request.header(IF_MATCH, &format!("\"{on}\"")),
    }
}

fn read(told: NoticeDTO) -> Result<RoleChange, ApiError> {
    let AboutDTO::RoleChanged { project, role, by } = told.about;

    Ok(RoleChange {
        noted_at: OffsetDateTime::parse(&told.noted_at, &Rfc3339)
            .map_err(|_| ApiError::Unexpected)?,
        role: Role::of(&role).ok_or(ApiError::Unexpected)?,
        project_name: told.names.get(&project).cloned(),
        by_name: by.and_then(|by| told.names.get(&by).cloned()),
        id: told.id,
        version: told.version,
    })
}

pub async fn open() -> Result<Vec<RoleChange>, ApiError> {
    let told: Vec<NoticeDTO> =
        http::parsed(http::sent(Request::get(NOTIFICATIONS)).await?, NOTICE).await?;

    told.into_iter().map(read).collect()
}

pub async fn dismiss(notice: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(made_on(
            Request::post(&format!("{NOTIFICATIONS}/{notice}/dismissed")),
            on,
        ))
        .await?,
        NOTICE,
    )
    .await
    .map(|_| ())
}
