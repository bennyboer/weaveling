use gloo_net::http::Request;
use passages_contract::{CreatePassageRequest, LinkIdeaRequest, PassageDTO, RetitlePassageRequest};

use crate::http::{ApiError, parsed};
use crate::ideas::model::IdeaId;
use crate::passages::model::{Passage, PassageId};
use crate::projects::model::ProjectId;

const PASSAGES: &str = "/api/passages";
const SUBJECT: &str = "passage";

pub async fn open(id: &PassageId) -> Result<Passage, ApiError> {
    let response = Request::get(&format!("{PASSAGES}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_passage(parsed(response, SUBJECT).await?))
}

pub async fn retitle(id: &PassageId, title: &str) -> Result<Passage, ApiError> {
    let payload = RetitlePassageRequest {
        title: title.to_owned(),
    };
    let response = Request::patch(&format!("{PASSAGES}/{id}"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_passage(parsed(response, SUBJECT).await?))
}

pub async fn link(id: &PassageId, idea: &IdeaId) -> Result<Passage, ApiError> {
    let payload = LinkIdeaRequest {
        idea: idea.to_string(),
    };
    let response = Request::post(&format!("{PASSAGES}/{id}/ideas"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_passage(parsed(response, SUBJECT).await?))
}

pub async fn unlink(id: &PassageId, idea: &IdeaId) -> Result<Passage, ApiError> {
    let response = Request::delete(&format!("{PASSAGES}/{id}/ideas/{idea}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_passage(parsed(response, SUBJECT).await?))
}

pub async fn in_project(project: &ProjectId) -> Result<Vec<Passage>, ApiError> {
    let response = Request::get(&format!("{PASSAGES}?project={project}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let listed: Vec<PassageDTO> = parsed(response, SUBJECT).await?;

    Ok(listed.into_iter().map(as_passage).collect())
}

pub async fn create(project: &ProjectId) -> Result<Passage, ApiError> {
    let payload = CreatePassageRequest {
        project: project.to_string(),
    };
    let response = Request::post(PASSAGES)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_passage(parsed(response, SUBJECT).await?))
}

fn as_passage(dto: PassageDTO) -> Passage {
    Passage {
        id: PassageId::from(dto.id),
        title: dto.title,
        ideas: dto.ideas.into_iter().map(IdeaId::from).collect(),
        text: dto.text,
    }
}
