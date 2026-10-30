use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing;
use boards_contract::{
    BoardDTO, OpenBoardRequest, PinIdeaRequest, PositionedIdeaDTO, ReshapeIdeaRequest, SizeDTO,
    SpotDTO, refused,
};
use boards_core::{
    Board, BoardError, BoardsCore, BoardsError, IdeaLink, PositionedIdea, Size, Spot,
};
use eventsourcing::{Agent, ServiceError, Standing, Version};
use live::{About, AccessWatch, Broadcaster, Gate, Refused, Subscribed, Topic};
use serving::Problem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardChanges(pub String);

pub struct BoardsTopic {
    boards: BoardsCore,
    broadcaster: Broadcaster<BoardChanges>,
}

impl BoardsTopic {
    pub fn new(boards: BoardsCore, broadcaster: Broadcaster<BoardChanges>) -> Self {
        Self {
            boards,
            broadcaster,
        }
    }
}

#[async_trait]
impl Topic for BoardsTopic {
    fn named(&self) -> &'static str {
        "boards"
    }

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        let Some(board) = about.get("board") else {
            return Err(Refused::NotThere);
        };
        let hearing = self.broadcaster.hear(BoardChanges(board.clone()));
        let admission =
            live::admitted(Arc::new(BoardsGate(self.boards.clone())), agent, board).await?;
        let standing = self
            .boards
            .read(board, agent)
            .await
            .map_err(|failed| refused(&failed))?;

        Ok(Subscribed::hearing(hearing)
            .standing_at(standing.version.count())
            .while_admitted(admission))
    }
}

fn refused(failed: &BoardsError) -> Refused {
    match failed {
        BoardsError::InvalidId(_)
        | BoardsError::ProjectNotThere
        | BoardsError::Events(ServiceError::NotFound { .. }) => Refused::NotThere,
        _ => Refused::Failed,
    }
}

struct BoardsGate(BoardsCore);

#[async_trait]
impl Gate for BoardsGate {
    async fn admits(
        &self,
        agent: &Agent,
        board: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        match self.0.read(board, agent).await {
            Ok(_) => Ok(true),
            Err(failed) => match refused(&failed) {
                Refused::NotThere => Ok(false),
                Refused::Failed => Err(Box::new(failed)),
            },
        }
    }

    async fn watch(&self, board: &str) -> AccessWatch {
        self.0
            .watch_access(board)
            .await
            .unwrap_or_else(|_| AccessWatch::still())
    }
}

pub fn router(boards: BoardsCore) -> Router {
    Router::new()
        .route("/boards", routing::post(open))
        .route("/boards/{board}", routing::get(find))
        .route("/boards/{board}/ideas", routing::post(pin))
        .route(
            "/boards/{board}/ideas/{idea}",
            routing::delete(unpin).patch(reshape),
        )
        .with_state(boards)
}

async fn open(
    State(boards): State<BoardsCore>,
    Extension(acting): Extension<Agent>,
    Json(request): Json<OpenBoardRequest>,
) -> Result<Response, Problem> {
    let opened = boards
        .open(&request.project, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(
        StatusCode::OK,
        &opened.id.to_string(),
        &opened.standing,
    ))
}

async fn find(
    State(boards): State<BoardsCore>,
    Extension(acting): Extension<Agent>,
    Path(board): Path<String>,
) -> Result<Response, Problem> {
    let found = boards.read(&board, &acting).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &board, &found))
}

async fn pin(
    State(boards): State<BoardsCore>,
    Extension(acting): Extension<Agent>,
    Path(board): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PinIdeaRequest>,
) -> Result<Response, Problem> {
    let written = boards
        .pin(
            &board,
            IdeaLink::from(request.idea.as_str()),
            as_spot(request.spot),
            as_size(request.size),
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &board, &written))
}

async fn reshape(
    State(boards): State<BoardsCore>,
    Extension(acting): Extension<Agent>,
    Path((board, idea)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<ReshapeIdeaRequest>,
) -> Result<Response, Problem> {
    let written = boards
        .reshape(
            &board,
            IdeaLink::from(idea.as_str()),
            request.spot.map(as_spot),
            request.size.map(as_size),
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &board, &written))
}

async fn unpin(
    State(boards): State<BoardsCore>,
    Extension(acting): Extension<Agent>,
    Path((board, idea)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    boards
        .unpin(
            &board,
            IdeaLink::from(idea.as_str()),
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn as_spot(at: SpotDTO) -> Spot {
    Spot::at(at.x, at.y)
}

fn as_size(size: SizeDTO) -> Size {
    Size::of(size.width, size.height)
}

fn to_response(status: StatusCode, board: &str, standing: &Standing<Board>) -> Response {
    (
        status,
        [(ETAG, serving::tag(standing.version))],
        Json(to_dto(board, &standing.state, standing.version)),
    )
        .into_response()
}

fn to_dto(board: &str, held: &Board, version: Version) -> BoardDTO {
    BoardDTO {
        id: board.to_owned(),
        version: version.count(),
        project: held.project().to_string(),
        ideas: held.ideas().iter().map(to_positioned_dto).collect(),
    }
}

fn to_positioned_dto(held: &PositionedIdea) -> PositionedIdeaDTO {
    PositionedIdeaDTO {
        idea: held.idea.to_string(),
        spot: SpotDTO {
            x: held.spot.x,
            y: held.spot.y,
        },
        size: SizeDTO {
            width: held.size.width,
            height: held.size.height,
        },
    }
}

fn problem_of(refusal: BoardsError) -> Problem {
    match refusal {
        BoardsError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        BoardsError::Events(events) => Problem::of(&events, |refusal| match refusal {
            BoardError::AlreadyPinned => Problem::refused(refused::ALREADY_PINNED, refusal),
            BoardError::NotPinned => Problem::not_found(refused::NOT_PINNED, refusal),
            BoardError::Shapeless => Problem::unprocessable(refused::SHAPELESS, refusal),
            BoardError::Discarded => Problem::refused(refused::DISCARDED, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        }),
        BoardsError::ProjectNotThere => {
            Problem::not_found(serving::NOT_FOUND, BoardsError::ProjectNotThere)
        }
        BoardsError::NotAnAuthor => {
            Problem::forbidden(refused::NOT_AN_AUTHOR, BoardsError::NotAnAuthor)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
