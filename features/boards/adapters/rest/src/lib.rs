use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
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
use eventsourcing::{Agent, Standing, Version};
use live::{Admission, Asked, Broadcaster, Gate};
use serving::Problem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardChanges(pub String);

#[derive(Clone)]
struct Following {
    broadcaster: Broadcaster<BoardChanges>,
    boards: BoardsCore,
}

pub fn live(boards: BoardsCore, broadcaster: Broadcaster<BoardChanges>) -> Router {
    Router::new()
        .route("/boards/live", routing::get(follow))
        .with_state(Following {
            broadcaster,
            boards,
        })
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(following): State<Following>,
    Extension(acting): Extension<Agent>,
    Query(asked): Query<Asked>,
) -> Response {
    let Some(board) = asked.get("board") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let hearing = following.broadcaster.hear(BoardChanges(board.clone()));

    match following.boards.follow(board, &acting).await {
        Ok((standing, watch)) => live::streaming_with_version(
            upgrade,
            standing.version.count(),
            hearing,
            Admission::new(
                watch,
                Arc::new(BoardsGate(following.boards)),
                acting,
                board.clone(),
            ),
        ),
        Err(refused) => problem_of(refused).into_response(),
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
        Ok(self.0.read(board, agent).await.is_ok())
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
