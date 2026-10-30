use gloo_net::http::Request;
use notifications_contract::{AboutDTO, NoticeDTO};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http;
use crate::http::ApiError;
use crate::members::listed::Role;
use crate::notifications::notice::RoleChange;

const NOTIFICATIONS: &str = "/api/notifications";
const NOTICE: &str = "notice";

fn read(told: NoticeDTO) -> Result<RoleChange, ApiError> {
    let AboutDTO::RoleChanged { project, role, by } = told.about;

    Ok(RoleChange {
        noted_at: OffsetDateTime::parse(&told.noted_at, &Rfc3339)
            .map_err(|_| ApiError::Unexpected)?,
        role: Role::of(&role).ok_or(ApiError::Unexpected)?,
        project_name: told.names.get(&project).cloned(),
        by_name: by.and_then(|by| told.names.get(&by).cloned()),
        id: told.id,
    })
}

pub async fn open() -> Result<Vec<RoleChange>, ApiError> {
    let told: Vec<NoticeDTO> =
        http::parsed(http::sent(Request::get(NOTIFICATIONS)).await?, NOTICE).await?;

    told.into_iter().map(read).collect()
}

pub async fn dismiss(notice: &str) -> Result<(), ApiError> {
    http::checked(
        http::sent(Request::post(&format!(
            "{NOTIFICATIONS}/{notice}/dismissed"
        )))
        .await?,
        NOTICE,
    )
    .await
    .map(|_| ())
}
