use boards_contract::{
    BoardDTO, OpenBoardRequest, PinPieceRequest, PositionedPieceDTO, ReshapePieceRequest, SizeDTO,
    SpotDTO,
};
use gloo_net::http::Request;

use crate::boards::model::{Board, BoardId, Placement, PositionedPiece, Size, Spot};
use crate::http::{ApiError, parsed};
use crate::pieces::model::PieceId;
use crate::projects::model::ProjectId;

const BOARDS: &str = "/api/boards";
const SUBJECT: &str = "board";

pub async fn open(project: &ProjectId) -> Result<Board, ApiError> {
    let payload = OpenBoardRequest {
        project: project.to_string(),
    };
    let response = Request::post(BOARDS)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_board(parsed(response, SUBJECT).await?))
}

pub async fn pin(board: &BoardId, piece: &PieceId, at: Placement) -> Result<Board, ApiError> {
    let payload = PinPieceRequest {
        piece: piece.to_string(),
        spot: to_spot_dto(at.spot),
        size: to_size_dto(at.size),
    };
    let response = Request::post(&format!("{BOARDS}/{board}/pieces"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_board(parsed(response, SUBJECT).await?))
}

pub async fn reshape(
    board: &BoardId,
    piece: &PieceId,
    to: Option<Spot>,
    size: Option<Size>,
) -> Result<Board, ApiError> {
    let payload = ReshapePieceRequest {
        spot: to.map(to_spot_dto),
        size: size.map(to_size_dto),
    };
    let response = Request::patch(&format!("{BOARDS}/{board}/pieces/{piece}"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_board(parsed(response, SUBJECT).await?))
}

pub async fn unpin(board: &BoardId, piece: &PieceId) -> Result<(), ApiError> {
    let response = Request::delete(&format!("{BOARDS}/{board}/pieces/{piece}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    if response.ok() {
        return Ok(());
    }

    Err(ApiError::Unexpected)
}

fn as_board(dto: BoardDTO) -> Board {
    Board {
        id: BoardId::from(dto.id),
        version: dto.version,
        pieces: dto.pieces.iter().map(as_positioned).collect(),
    }
}

fn as_positioned(dto: &PositionedPieceDTO) -> PositionedPiece {
    PositionedPiece {
        piece: PieceId::from(dto.piece.clone()),
        spot: Spot {
            x: dto.spot.x,
            y: dto.spot.y,
        },
        size: Size {
            width: dto.size.width,
            height: dto.size.height,
        },
    }
}

fn to_spot_dto(at: Spot) -> SpotDTO {
    SpotDTO { x: at.x, y: at.y }
}

fn to_size_dto(size: Size) -> SizeDTO {
    SizeDTO {
        width: size.width,
        height: size.height,
    }
}
