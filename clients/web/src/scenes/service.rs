use gloo_net::http::{Request, RequestBuilder, Response};
use scenes_contract::{
    CreateSceneRequest, LinkIdeaRequest, QuotationRestylingDTO, RetitleSceneRequest, SceneDTO,
    ScenePageDTO,
};

use crate::http;
use crate::http::ApiError;
use crate::ideas::model::IdeaId;
use crate::projects::model::ProjectId;
use crate::scenes::model::{Scene, SceneId, SceneSummary};
use crate::scenes::restyling::QuotationRestyling;
use crate::scenes::written;

const SCENES: &str = "/api/scenes";
const SUBJECT: &str = "scene";
const RESTYLING: &str = "rewriting of the quotation marks";

pub async fn open(id: &SceneId) -> Result<Scene, ApiError> {
    let response = http::sent(Request::get(&format!("{SCENES}/{id}"))).await?;

    told(response).await
}

pub async fn retitle(id: &SceneId, title: &str, on: u64) -> Result<Scene, ApiError> {
    let payload = RetitleSceneRequest {
        title: title.to_owned(),
    };
    let response = http::sent_with(
        made_on(Request::patch(&format!("{SCENES}/{id}")), on),
        &payload,
    )
    .await?;

    told(response).await
}

pub async fn link(id: &SceneId, idea: &IdeaId, on: u64) -> Result<Scene, ApiError> {
    let payload = LinkIdeaRequest {
        idea: idea.to_string(),
    };
    let response = http::sent_with(
        made_on(Request::post(&format!("{SCENES}/{id}/ideas")), on),
        &payload,
    )
    .await?;

    told(response).await
}

pub async fn unlink(id: &SceneId, idea: &IdeaId, on: u64) -> Result<Scene, ApiError> {
    let response = http::sent(made_on(
        Request::delete(&format!("{SCENES}/{id}/ideas/{idea}")),
        on,
    ))
    .await?;

    told(response).await
}

pub async fn delete(id: &SceneId, on: u64) -> Result<(), ApiError> {
    let response = http::sent(made_on(Request::delete(&format!("{SCENES}/{id}")), on)).await?;
    http::checked(response, SUBJECT).await?;
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
        let response = http::sent(Request::get(&asked)).await?;
        let page: ScenePageDTO = http::parsed(response, SUBJECT).await?;
        listed.extend(page.scenes.into_iter().map(SceneSummary::from));

        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(written::caught_up(project, listed)),
        }
    }
}

pub async fn quotation_restyling(
    project: &ProjectId,
) -> Result<Option<QuotationRestyling>, ApiError> {
    let response = http::sent(Request::get(&format!(
        "{SCENES}/quotation-restylings/{project}"
    )))
    .await?;
    if response.status() == 404 {
        return Ok(None);
    }
    let found: QuotationRestylingDTO = http::parsed(response, RESTYLING).await?;

    Ok(Some(QuotationRestyling::from(found)))
}

pub async fn create(project: &ProjectId) -> Result<Scene, ApiError> {
    let payload = CreateSceneRequest {
        project: project.to_string(),
    };
    let response = http::sent_with(Request::post(SCENES), &payload).await?;
    let made = told(response).await?;
    written::remember(project, made.summarized());

    Ok(made)
}

async fn told(response: Response) -> Result<Scene, ApiError> {
    let dto: SceneDTO = http::parsed(response, SUBJECT).await?;

    Ok(Scene {
        id: SceneId::from(dto.id),
        title: dto.title,
        ideas: dto.ideas.into_iter().map(IdeaId::from).collect(),
        text: dto.text,
        version: dto.version,
    })
}

fn made_on(request: RequestBuilder, on: u64) -> RequestBuilder {
    request.header("If-Match", &format!("\"{on}\""))
}
