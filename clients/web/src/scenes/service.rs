use gloo_net::http::Request;
use scenes_contract::{
    CreateSceneRequest, LinkIdeaRequest, RetitleSceneRequest, SceneDTO, ScenePageDTO,
    SceneSummaryDTO,
};

use crate::http::{ApiError, checked, parsed};
use crate::ideas::model::IdeaId;
use crate::projects::model::ProjectId;
use crate::scenes::model::{Scene, SceneId, SceneSummary};

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

pub async fn delete(id: &SceneId) -> Result<(), ApiError> {
    let response = Request::delete(&format!("{SCENES}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    checked(response, SUBJECT).await.map(|_| ())
}

pub async fn in_project(project: &ProjectId) -> Result<Vec<SceneSummary>, ApiError> {
    let mut listed = Vec::new();
    let mut after: Option<String> = None;

    loop {
        let asked = match &after {
            Some(last) => format!("{SCENES}?project={project}&after={last}"),
            None => format!("{SCENES}?project={project}"),
        };
        let response = Request::get(&asked)
            .send()
            .await
            .map_err(|_| ApiError::Offline)?;
        let page: ScenePageDTO = parsed(response, SUBJECT).await?;
        listed.extend(page.scenes.into_iter().map(as_summary));

        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(listed),
        }
    }
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

fn as_summary(dto: SceneSummaryDTO) -> SceneSummary {
    SceneSummary {
        id: SceneId::from(dto.id),
        title: dto.title,
        opening: dto.opening,
    }
}
