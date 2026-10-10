use gloo_net::http::Request;
use scenes_contract::{CreateSceneRequest, LinkIdeaRequest, RetitleSceneRequest, SceneDTO};

use crate::http::{ApiError, parsed};
use crate::ideas::model::IdeaId;
use crate::projects::model::ProjectId;
use crate::scenes::model::{Scene, SceneId};

const SCENES: &str = "/api/scenes";
const SUBJECT: &str = "scene";

pub async fn open(id: &SceneId) -> Result<Scene, ApiError> {
    let response = Request::get(&format!("{SCENES}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

pub async fn retitle(id: &SceneId, title: &str) -> Result<Scene, ApiError> {
    let payload = RetitleSceneRequest {
        title: title.to_owned(),
    };
    let response = Request::patch(&format!("{SCENES}/{id}"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

pub async fn link(id: &SceneId, idea: &IdeaId) -> Result<Scene, ApiError> {
    let payload = LinkIdeaRequest {
        idea: idea.to_string(),
    };
    let response = Request::post(&format!("{SCENES}/{id}/ideas"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

pub async fn unlink(id: &SceneId, idea: &IdeaId) -> Result<Scene, ApiError> {
    let response = Request::delete(&format!("{SCENES}/{id}/ideas/{idea}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

pub async fn in_project(project: &ProjectId) -> Result<Vec<Scene>, ApiError> {
    let response = Request::get(&format!("{SCENES}?project={project}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let listed: Vec<SceneDTO> = parsed(response, SUBJECT).await?;

    Ok(listed.into_iter().map(as_scene).collect())
}

pub async fn create(project: &ProjectId) -> Result<Scene, ApiError> {
    let payload = CreateSceneRequest {
        project: project.to_string(),
    };
    let response = Request::post(SCENES)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

fn as_scene(dto: SceneDTO) -> Scene {
    Scene {
        id: SceneId::from(dto.id),
        title: dto.title,
        ideas: dto.ideas.into_iter().map(IdeaId::from).collect(),
        text: dto.text,
    }
}
