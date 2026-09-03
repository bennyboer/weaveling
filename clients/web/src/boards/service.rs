use boards_contract::{BoardDTO, OpenBoardRequest, PinPieceRequest, PositionedPieceDTO, SpotDTO};
use gloo_net::http::Request;

use crate::boards::model::{Board, BoardId, PositionedPiece, Spot};
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

pub async fn pin(board: &BoardId, piece: &PieceId, at: Spot) -> Result<Board, ApiError> {
    let payload = PinPieceRequest {
        piece: piece.to_string(),
        spot: SpotDTO { x: at.x, y: at.y },
    };
    let response = Request::post(&format!("{BOARDS}/{board}/pieces"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_board(parsed(response, SUBJECT).await?))
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
    }
}
