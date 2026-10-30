use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing;
use eventsourcing::{Agent, Standing};
use ideas_contract::{CaptureIdeaRequest, IdeaDTO, RetitleIdeaRequest, refused};
use ideas_core::{
    Idea, IdeaError, IdeaSummary, IdeasCore, IdeasError, InvalidIdeaTitle, ProjectLink,
};
use live::{AccessWatch, Broadcaster, Gate};
use serde::Deserialize;
use serving::Problem;

struct IdeasGate(IdeasCore);

#[async_trait]
impl Gate for IdeasGate {
    async fn admits(
        &self,
        agent: &Agent,
        project: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(self.0.may_follow(project, agent).await?)
    }

    fn watch(&self, project: &str) -> AccessWatch {
        self.0.watch_access(project)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeaChanges(pub ProjectLink);

impl Display for IdeaChanges {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub fn live(ideas: IdeasCore, broadcaster: Broadcaster<IdeaChanges>) -> Router {
    live::followed_while(
        "/ideas/live",
        broadcaster,
        |asked| {
            asked
                .get("project")
                .map(|project| IdeaChanges(ProjectLink::from(project.as_str())))
        },
        Arc::new(IdeasGate(ideas)),
    )
}

pub fn router(ideas: IdeasCore) -> Router {
    Router::new()
        .route("/ideas", routing::get(list).post(capture))
        .route(
            "/ideas/{id}",
            routing::get(find).patch(retitle).delete(discard),
        )
        .with_state(ideas)
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn list(
    State(ideas): State<IdeasCore>,
    Extension(acting): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<IdeaDTO>>, Problem> {
    let found = ideas
        .list(&asked.project, &acting)
        .await
        .map_err(problem_of)?;

    Ok(Json(found.iter().map(to_dto).collect()))
}

fn to_dto(summary: &IdeaSummary) -> IdeaDTO {
    IdeaDTO {
        id: summary.id.to_string(),
        version: summary.version.count(),
        project: summary.project.to_string(),
        title: summary.title.to_string(),
    }
}

async fn capture(
    State(ideas): State<IdeasCore>,
    Extension(acting): Extension<Agent>,
    Json(request): Json<CaptureIdeaRequest>,
) -> Result<Response, Problem> {
    let (id, captured) = ideas
        .capture(&request.project, &request.title, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::CREATED, &id.to_string(), &captured))
}

async fn find(
    State(ideas): State<IdeasCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
) -> Result<Response, Problem> {
    let found = ideas.read(&id, &acting).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &id, &found))
}

async fn retitle(
    State(ideas): State<IdeasCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RetitleIdeaRequest>,
) -> Result<Response, Problem> {
    let retitled = ideas
        .retitle(&id, &request.title, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &id, &retitled))
}

async fn discard(
    State(ideas): State<IdeasCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    ideas
        .discard(&id, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_response(status: StatusCode, id: &str, standing: &Standing<Idea>) -> Response {
    let idea = &standing.state;

    (
        status,
        [(ETAG, serving::tag(standing.version))],
        Json(IdeaDTO {
            id: id.to_owned(),
            version: standing.version.count(),
            project: idea.project().to_string(),
            title: idea.title().to_string(),
        }),
    )
        .into_response()
}

fn problem_of(refusal: IdeasError) -> Problem {
    match refusal {
        IdeasError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        IdeasError::InvalidTitle(reason) => match reason {
            InvalidIdeaTitle::Blank => Problem::unexpected(reason),
            InvalidIdeaTitle::ControlCharacter => {
                Problem::unprocessable(refused::TITLE_CONTROL_CHARACTER, reason)
            }
            InvalidIdeaTitle::TooLong { max, actual } => {
                Problem::unprocessable(refused::TITLE_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        IdeasError::Events(events) => Problem::of(&events, |refusal| match refusal {
            IdeaError::Discarded => Problem::refused(refused::DISCARDED, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        }),
        IdeasError::ProjectNotThere => {
            Problem::not_found(serving::NOT_FOUND, IdeasError::ProjectNotThere)
        }
        IdeasError::NotAnAuthor => {
            Problem::forbidden(refused::NOT_AN_AUTHOR, IdeasError::NotAnAuthor)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
