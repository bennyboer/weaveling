use gloo_net::http::Request;
use scenes_contract::{
    CreateSceneRequest, LinkIdeaRequest, QuotationRestylingDTO, RetitleSceneRequest, SceneDTO,
    ScenePageDTO, SceneSummaryDTO,
};

use crate::http::{ApiError, checked, parsed, sent, sent_with};
use crate::ideas::model::IdeaId;
use crate::projects::model::ProjectId;
use crate::scenes::model::{Scene, SceneId, SceneSummary};
use crate::scenes::restyling::QuotationRestyling;
use crate::scenes::written;

const SCENES: &str = "/api/scenes";
const SUBJECT: &str = "scene";
const RESTYLING: &str = "rewriting of the quotation marks";

pub async fn open(id: &SceneId) -> Result<Scene, ApiError> {
    let response = sent(Request::get(&format!("{SCENES}/{id}"))).await?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

pub async fn retitle(id: &SceneId, title: &str) -> Result<Scene, ApiError> {
    let payload = RetitleSceneRequest {
        title: title.to_owned(),
    };
    let response = sent_with(Request::patch(&format!("{SCENES}/{id}")), &payload).await?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

pub async fn link(id: &SceneId, idea: &IdeaId) -> Result<Scene, ApiError> {
    let payload = LinkIdeaRequest {
        idea: idea.to_string(),
    };
    let response = sent_with(Request::post(&format!("{SCENES}/{id}/ideas")), &payload).await?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

pub async fn unlink(id: &SceneId, idea: &IdeaId) -> Result<Scene, ApiError> {
    let response = sent(Request::delete(&format!("{SCENES}/{id}/ideas/{idea}"))).await?;

    Ok(as_scene(parsed(response, SUBJECT).await?))
}

pub async fn delete(id: &SceneId) -> Result<(), ApiError> {
    let response = sent(Request::delete(&format!("{SCENES}/{id}"))).await?;
    checked(response, SUBJECT).await?;
    written::forget(id);

    Ok(())
}

pub async fn in_project(project: &ProjectId) -> Result<Vec<SceneSummary>, ApiError> {
    let mut listed = Vec::new();
    let mut after: Option<String> = None;

    loop {
        let asked = match &after {
            Some(last) => format!("{SCENES}?project={project}&after={last}"),
            None => format!("{SCENES}?project={project}"),
        };
        let response = sent(Request::get(&asked)).await?;
        let page: ScenePageDTO = parsed(response, SUBJECT).await?;
        listed.extend(page.scenes.into_iter().map(as_summary));

        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(written::caught_up(project, listed)),
        }
    }
}

pub async fn quotation_restyling(
    project: &ProjectId,
) -> Result<Option<QuotationRestyling>, ApiError> {
    let response = sent(Request::get(&format!(
        "{SCENES}/quotation-restylings/{project}"
    )))
    .await?;
    if response.status() == 404 {
        return Ok(None);
    }
    let found: QuotationRestylingDTO = parsed(response, RESTYLING).await?;

    Ok(Some(QuotationRestyling::from(found)))
}

pub async fn create(project: &ProjectId) -> Result<Scene, ApiError> {
    let payload = CreateSceneRequest {
        project: project.to_string(),
    };
    let response = sent_with(Request::post(SCENES), &payload).await?;
    let made = as_scene(parsed(response, SUBJECT).await?);
    written::remember(project, made.summarized());

    Ok(made)
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
