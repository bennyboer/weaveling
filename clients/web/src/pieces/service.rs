use gloo_net::http::Request;
use pieces_contract::{AttachPassageRequest, CapturePieceRequest, PieceDTO, RetitlePieceRequest};

use crate::http::{ApiError, parsed};
use crate::passages::model::PassageId;
use crate::pieces::model::{Piece, PieceId};
use crate::projects::model::ProjectId;

const PIECES: &str = "/api/pieces";
const SUBJECT: &str = "piece";

pub async fn list(project: &ProjectId) -> Result<Vec<Piece>, ApiError> {
    let response = Request::get(&format!("{PIECES}?project={project}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let listed: Vec<PieceDTO> = parsed(response, SUBJECT).await?;

    Ok(listed.into_iter().map(as_piece).collect())
}

pub async fn capture(project: &ProjectId, title: &str) -> Result<Piece, ApiError> {
    let payload = CapturePieceRequest {
        project: project.to_string(),
        title: title.to_owned(),
    };
    let response = Request::post(PIECES)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_piece(parsed(response, SUBJECT).await?))
}

pub async fn retitle(id: &PieceId, title: &str) -> Result<Piece, ApiError> {
    let payload = RetitlePieceRequest {
        title: title.to_owned(),
    };
    let response = Request::patch(&format!("{PIECES}/{id}"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_piece(parsed(response, SUBJECT).await?))
}

fn as_piece(dto: PieceDTO) -> Piece {
    Piece {
        id: PieceId::from(dto.id),
        version: dto.version,
        title: dto.title,
        passage: dto.passage.map(PassageId::from),
    }
}

pub async fn get(id: &PieceId) -> Result<Piece, ApiError> {
    let response = Request::get(&format!("{PIECES}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_piece(parsed(response, SUBJECT).await?))
}

pub async fn attach_passage(id: &PieceId, passage: &PassageId) -> Result<Piece, ApiError> {
    let payload = AttachPassageRequest {
        passage: passage.to_string(),
    };
    let response = Request::put(&format!("{PIECES}/{id}/passage"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_piece(parsed(response, SUBJECT).await?))
}
