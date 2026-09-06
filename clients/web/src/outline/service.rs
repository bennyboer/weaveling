use gloo_net::http::Request;
use outline_contract::{
    AddSectionRequest, AddedSectionResponse, AttachPieceRequest, MoveSectionRequest,
    OpenOutlineRequest, OutlineDTO, PlacedSectionDTO, RetitleSectionRequest,
};

use crate::http::{ApiError, parsed};
use crate::outline::model::{Outline, OutlineId, Section, SectionId};
use crate::pieces::model::PieceId;
use crate::projects::model::ProjectId;

const OUTLINES: &str = "/api/outlines";
const SUBJECT: &str = "outline";

pub async fn open(project: &ProjectId) -> Result<Outline, ApiError> {
    let payload = OpenOutlineRequest {
        project: project.to_string(),
    };
    let response = Request::post(OUTLINES)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_outline(parsed(response, SUBJECT).await?))
}

pub async fn add(
    outline: &OutlineId,
    under: Option<SectionId>,
    after: Option<SectionId>,
    title: &str,
) -> Result<(SectionId, Outline), ApiError> {
    let payload = AddSectionRequest {
        under: under.map(|under| under.to_string()),
        after: after.map(|after| after.to_string()),
        title: title.to_owned(),
    };
    let response = Request::post(&format!("{OUTLINES}/{outline}/sections"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let added: AddedSectionResponse = parsed(response, SUBJECT).await?;

    Ok((SectionId::from(added.section), as_outline(added.outline)))
}

pub async fn retitle(
    outline: &OutlineId,
    section: &SectionId,
    title: &str,
) -> Result<Outline, ApiError> {
    let payload = RetitleSectionRequest {
        title: title.to_owned(),
    };
    let response = Request::patch(&format!("{OUTLINES}/{outline}/sections/{section}"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_outline(parsed(response, SUBJECT).await?))
}

pub async fn place(
    outline: &OutlineId,
    section: &SectionId,
    under: Option<SectionId>,
    after: Option<SectionId>,
) -> Result<Outline, ApiError> {
    let payload = MoveSectionRequest {
        under: under.map(|under| under.to_string()),
        after: after.map(|after| after.to_string()),
    };
    let response = Request::put(&format!("{OUTLINES}/{outline}/sections/{section}/place"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_outline(parsed(response, SUBJECT).await?))
}

pub async fn promote(outline: &OutlineId, section: &SectionId) -> Result<Outline, ApiError> {
    urged(outline, section, "promote").await
}

pub async fn demote(outline: &OutlineId, section: &SectionId) -> Result<Outline, ApiError> {
    urged(outline, section, "demote").await
}

pub async fn remove(outline: &OutlineId, section: &SectionId) -> Result<Outline, ApiError> {
    let response = Request::delete(&format!("{OUTLINES}/{outline}/sections/{section}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_outline(parsed(response, SUBJECT).await?))
}

pub async fn attach(
    outline: &OutlineId,
    piece: &PieceId,
    to: &SectionId,
    after: Option<PieceId>,
) -> Result<Outline, ApiError> {
    let payload = AttachPieceRequest {
        piece: piece.to_string(),
        section: to.to_string(),
        after: after.map(|after| after.to_string()),
    };
    let response = Request::post(&format!("{OUTLINES}/{outline}/pieces"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_outline(parsed(response, SUBJECT).await?))
}

pub async fn detach(outline: &OutlineId, piece: &PieceId) -> Result<(), ApiError> {
    let response = Request::delete(&format!("{OUTLINES}/{outline}/pieces/{piece}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    if response.ok() {
        return Ok(());
    }

    Err(ApiError::Unexpected)
}

async fn urged(outline: &OutlineId, section: &SectionId, how: &str) -> Result<Outline, ApiError> {
    let response = Request::post(&format!("{OUTLINES}/{outline}/sections/{section}/{how}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    Ok(as_outline(parsed(response, SUBJECT).await?))
}

fn as_outline(dto: OutlineDTO) -> Outline {
    Outline {
        id: OutlineId::from(dto.id),
        version: dto.version,
        sections: dto.sections.into_iter().map(as_section).collect(),
    }
}

fn as_section(dto: PlacedSectionDTO) -> Section {
    Section {
        id: SectionId::from(dto.section),
        parent: dto.parent.map(SectionId::from),
        title: dto.title,
        pieces: dto.pieces.into_iter().map(PieceId::from).collect(),
    }
}
