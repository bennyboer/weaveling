use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use boards_contract::{
    BoardDTO, MovePieceRequest, OpenBoardRequest, PinPieceRequest, PositionedPieceDTO, SpotDTO,
};
use boards_core::{
    Board, BoardError, BoardService, BoardServiceError, PieceLink, PositionedPiece, Spot,
};
use eventsourcing::{Agent, ServiceError, Standing, Version};
use serving::{Unreadable, demanded, refusal, tag};

pub fn router(boards: BoardService) -> Router {
    Router::new()
        .route("/boards", post(open))
        .route("/boards/{board}", get(find))
        .route("/boards/{board}/pieces", post(pin))
        .route(
            "/boards/{board}/pieces/{piece}",
            delete(unpin).patch(move_piece),
        )
        .with_state(boards)
}

async fn open(
    State(boards): State<BoardService>,
    Json(request): Json<OpenBoardRequest>,
) -> Result<Response, ApiError> {
    let opened = boards.open(&request.project, &nobody_yet()).await?;

    Ok(reported(
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

    Ok(reported(StatusCode::OK, &board, &found))
}

async fn pin(
    State(boards): State<BoardService>,
    Path(board): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PinPieceRequest>,
) -> Result<Response, ApiError> {
    boards
        .pin(
            &board,
            PieceLink::from(request.piece.as_str()),
            spot(request.spot),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(reported(StatusCode::OK, &board, &boards.get(&board).await?))
}

async fn move_piece(
    State(boards): State<BoardService>,
    Path((board, piece)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<MovePieceRequest>,
) -> Result<Response, ApiError> {
    boards
        .move_piece(
            &board,
            PieceLink::from(piece.as_str()),
            spot(request.spot),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(reported(StatusCode::OK, &board, &boards.get(&board).await?))
}

async fn unpin(
    State(boards): State<BoardService>,
    Path((board, piece)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    boards
        .unpin(
            &board,
            PieceLink::from(piece.as_str()),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn nobody_yet() -> Agent {
    Agent::Anonymous
}

fn spot(at: SpotDTO) -> Spot {
    Spot::at(at.x, at.y)
}

fn expected(headers: &HeaderMap) -> Result<Option<Version>, ApiError> {
    Ok(demanded(headers)?)
}

fn reported(status: StatusCode, board: &str, standing: &Standing<Board>) -> Response {
    (
        status,
        [(ETAG, tag(standing.version))],
        Json(as_dto(board, &standing.state, standing.version)),
    )
        .into_response()
}

fn as_dto(board: &str, held: &Board, version: Version) -> BoardDTO {
    BoardDTO {
        id: board.to_owned(),
        version: version.count(),
        project: held.project().to_string(),
        pieces: held.pieces().iter().map(positioned).collect(),
    }
}

fn positioned(held: &PositionedPiece) -> PositionedPieceDTO {
    PositionedPieceDTO {
        piece: held.piece.to_string(),
        spot: SpotDTO {
            x: held.spot.x,
            y: held.spot.y,
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
        let refused = match self {
            Self::Unreadable(reason) => {
                return (StatusCode::BAD_REQUEST, reason.to_string()).into_response();
            }
            Self::Refused(refused) => refused,
        };

        let (status, message) = match refused {
            BoardServiceError::InvalidId(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            BoardServiceError::Events(ServiceError::Refused(BoardError::NotPinned)) => {
                (StatusCode::NOT_FOUND, BoardError::NotPinned.to_string())
            }
            BoardServiceError::Events(events) => refusal(&events),
            unserveable => {
                tracing::error!(error = %unserveable, "a board request could not be served");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
