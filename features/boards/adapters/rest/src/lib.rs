use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use boards_contract::{
    BoardDTO, OpenBoardRequest, PinIdeaRequest, PositionedIdeaDTO, ReshapeIdeaRequest, SizeDTO,
    SpotDTO, refused,
};
use boards_core::{
    Board, BoardError, BoardService, BoardServiceError, IdeaLink, PositionedIdea, Size, Spot,
};
use eventsourcing::{Agent, Standing, Version};
use live::{Broadcaster, followed_from};
use serving::{Problem, UNREADABLE, Unreadable, demanded, tag};

pub fn live(broadcaster: Broadcaster, boards: BoardService) -> Router {
    followed_from(
        "/boards/live",
        broadcaster,
        |asked| asked.get("board").cloned(),
        move |board: String| {
            let boards = boards.clone();
            async move {
                boards
                    .get(&board)
                    .await
                    .ok()
                    .map(|standing| standing.version.count())
            }
        },
    )
}

pub fn router(boards: BoardService) -> Router {
    Router::new()
        .route("/boards", post(open))
        .route("/boards/{board}", get(find))
        .route("/boards/{board}/ideas", post(pin))
        .route("/boards/{board}/ideas/{idea}", delete(unpin).patch(reshape))
        .with_state(boards)
}

async fn open(
    State(boards): State<BoardService>,
    Json(request): Json<OpenBoardRequest>,
) -> Result<Response, ApiError> {
    let opened = boards.open(&request.project, &nobody_yet()).await?;

    Ok(to_response(
        StatusCode::OK,
        &opened.id.to_string(),
        &opened.standing,
    ))
}

async fn find(
    State(boards): State<BoardService>,
    Path(board): Path<String>,
) -> Result<Response, ApiError> {
    let found = boards.get(&board).await?;

    Ok(to_response(StatusCode::OK, &board, &found))
}

async fn pin(
    State(boards): State<BoardService>,
    Path(board): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PinIdeaRequest>,
) -> Result<Response, ApiError> {
    boards
        .pin(
            &board,
            IdeaLink::from(request.idea.as_str()),
            as_spot(request.spot),
            as_size(request.size),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(to_response(
        StatusCode::OK,
        &board,
        &boards.get(&board).await?,
    ))
}

async fn reshape(
    State(boards): State<BoardService>,
    Path((board, idea)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<ReshapeIdeaRequest>,
) -> Result<Response, ApiError> {
    boards
        .reshape(
            &board,
            IdeaLink::from(idea.as_str()),
            request.spot.map(as_spot),
            request.size.map(as_size),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(to_response(
        StatusCode::OK,
        &board,
        &boards.get(&board).await?,
    ))
}

async fn unpin(
    State(boards): State<BoardService>,
    Path((board, idea)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    boards
        .unpin(
            &board,
            IdeaLink::from(idea.as_str()),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn nobody_yet() -> Agent {
    Agent::Anonymous
}

fn as_spot(at: SpotDTO) -> Spot {
    Spot::at(at.x, at.y)
}

fn as_size(size: SizeDTO) -> Size {
    Size::of(size.width, size.height)
}

fn expected(headers: &HeaderMap) -> Result<Option<Version>, ApiError> {
    Ok(demanded(headers)?)
}

fn to_response(status: StatusCode, board: &str, standing: &Standing<Board>) -> Response {
    (
        status,
        [(ETAG, tag(standing.version))],
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

enum ApiError {
    Unreadable(Unreadable),
    Refused(BoardServiceError),
}

impl From<BoardServiceError> for ApiError {
    fn from(error: BoardServiceError) -> Self {
        Self::Refused(error)
    }
}

impl From<Unreadable> for ApiError {
    fn from(error: Unreadable) -> Self {
        Self::Unreadable(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Unreadable(reason) => Problem::rejected(UNREADABLE, reason),
            Self::Refused(refusal) => problem_of(refusal),
        }
        .into_response()
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
        unserveable => Problem::unexpected(unserveable),
    }
}
