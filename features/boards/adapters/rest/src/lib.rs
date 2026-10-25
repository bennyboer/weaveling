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
    Board, BoardError, BoardService, BoardServiceError, IdeaLink, PositionedIdea, Size, Spot,
};
use eventsourcing::{Agent, Standing, Version};
use live::{Asked, Broadcaster};
use serving::Problem;

#[derive(Clone)]
struct Following {
    broadcaster: Broadcaster,
    boards: BoardService,
}

pub fn live(broadcaster: Broadcaster, boards: BoardService) -> Router {
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
    let hearing = following.broadcaster.hear(board);

    match following.boards.read(board, &acting).await {
        Ok(standing) => live::streaming_from(upgrade, standing.version.count(), hearing),
        Err(refused) => problem_of(refused).into_response(),
    }
}

pub fn router(boards: BoardService) -> Router {
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
    State(boards): State<BoardService>,
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
    State(boards): State<BoardService>,
    Extension(acting): Extension<Agent>,
    Path(board): Path<String>,
) -> Result<Response, Problem> {
    let found = boards.read(&board, &acting).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &board, &found))
}

async fn pin(
    State(boards): State<BoardService>,
    Extension(acting): Extension<Agent>,
    Path(board): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PinIdeaRequest>,
) -> Result<Response, Problem> {
    boards
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

    Ok(to_response(
        StatusCode::OK,
        &board,
        &boards.get(&board).await.map_err(problem_of)?,
    ))
}

async fn reshape(
    State(boards): State<BoardService>,
    Extension(acting): Extension<Agent>,
    Path((board, idea)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<ReshapeIdeaRequest>,
) -> Result<Response, Problem> {
    boards
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

    Ok(to_response(
        StatusCode::OK,
        &board,
        &boards.get(&board).await.map_err(problem_of)?,
    ))
}

async fn unpin(
    State(boards): State<BoardService>,
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

fn problem_of(refusal: BoardServiceError) -> Problem {
    match refusal {
        BoardServiceError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        BoardServiceError::Events(events) => Problem::of(&events, |refusal| match refusal {
            BoardError::AlreadyPinned => Problem::refused(refused::ALREADY_PINNED, refusal),
            BoardError::NotPinned => Problem::not_found(refused::NOT_PINNED, refusal),
            BoardError::Shapeless => Problem::unprocessable(refused::SHAPELESS, refusal),
            BoardError::Discarded => Problem::refused(refused::DISCARDED, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        }),
        BoardServiceError::ProjectNotThere => {
            Problem::not_found(serving::NOT_FOUND, BoardServiceError::ProjectNotThere)
        }
        BoardServiceError::NotAnAuthor => {
            Problem::forbidden(refused::NOT_AN_AUTHOR, BoardServiceError::NotAnAuthor)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
