use gloo_net::http::Request;
use ideas_contract::{AttachPassageRequest, CaptureIdeaRequest, IdeaDTO, RetitleIdeaRequest};

use crate::http::{ApiError, parsed};
use crate::ideas::model::{Idea, IdeaId};
use crate::passages::model::PassageId;
use crate::projects::model::ProjectId;

const IDEAS: &str = "/api/ideas";
const SUBJECT: &str = "idea";

pub async fn list(project: &ProjectId) -> Result<Vec<Idea>, ApiError> {
    let response = Request::get(&format!("{IDEAS}?project={project}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let listed: Vec<IdeaDTO> = parsed(response, SUBJECT).await?;

    Ok(listed.into_iter().map(as_idea).collect())
}

pub async fn capture(project: &ProjectId, title: &str) -> Result<Idea, ApiError> {
    let payload = CaptureIdeaRequest {
        project: project.to_string(),
        title: title.to_owned(),
    };
    let response = Request::post(IDEAS)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_idea(parsed(response, SUBJECT).await?))
}

pub async fn retitle(id: &IdeaId, title: &str) -> Result<Idea, ApiError> {
    let payload = RetitleIdeaRequest {
        title: title.to_owned(),
    };
    let response = Request::patch(&format!("{IDEAS}/{id}"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_idea(parsed(response, SUBJECT).await?))
}

fn as_idea(dto: IdeaDTO) -> Idea {
    Idea {
        id: IdeaId::from(dto.id),
        version: dto.version,
        title: dto.title,
        passage: dto.passage.map(PassageId::from),
    }
}

pub async fn get(id: &IdeaId) -> Result<Idea, ApiError> {
    let response = Request::get(&format!("{IDEAS}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_idea(parsed(response, SUBJECT).await?))
}

pub async fn attach_passage(id: &IdeaId, passage: &PassageId) -> Result<Idea, ApiError> {
    let payload = AttachPassageRequest {
        passage: passage.to_string(),
    };
    let response = Request::put(&format!("{IDEAS}/{id}/passage"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_idea(parsed(response, SUBJECT).await?))
}
