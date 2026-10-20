use gloo_net::http::{Request, Response};
use outline_contract::{
    AddSectionRequest, AddedSectionResponse, AttachRequest, AttachmentDTO, ChangedSectionDTO,
    MoveSectionRequest, OpenOutlineRequest, OutlineChangeDTO, OutlineDTO, PlacedSectionDTO,
    RetitleSectionRequest, refused,
};

use crate::http::{ApiError, checked, parsed, rejection, sent, sent_with};
use crate::ideas::model::IdeaId;
use crate::outline::model::{
    Attachment, Outline, OutlineChange, OutlineId, Section, SectionChange, SectionId,
};
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;

const OUTLINES: &str = "/api/outlines";
const SUBJECT: &str = "outline";

pub async fn open(project: &ProjectId) -> Result<Outline, ApiError> {
    let payload = OpenOutlineRequest {
        project: project.to_string(),
    };
    let response = sent_with(Request::post(OUTLINES), &payload).await?;

    Ok(as_outline(parsed(response, SUBJECT).await?))
}

pub async fn add(
    outline: &OutlineId,
    under: Option<SectionId>,
    after: Option<SectionId>,
    title: &str,
) -> Result<(SectionId, OutlineChange), ApiError> {
    let payload = AddSectionRequest {
        under: under.map(|under| under.to_string()),
        after: after.map(|after| after.to_string()),
        title: title.to_owned(),
    };
    let response = sent_with(
        Request::post(&format!("{OUTLINES}/{outline}/sections")),
        &payload,
    )
    .await?;
    let added: AddedSectionResponse = parsed(answered(response).await?, SUBJECT).await?;

    Ok((SectionId::from(added.section), as_change(added.outline)))
}

pub async fn retitle(
    outline: &OutlineId,
    section: &SectionId,
    title: &str,
) -> Result<OutlineChange, ApiError> {
    let payload = RetitleSectionRequest {
        title: title.to_owned(),
    };
    let response = sent_with(
        Request::patch(&format!("{OUTLINES}/{outline}/sections/{section}")),
        &payload,
    )
    .await?;

    changed(response).await
}

pub async fn place(
    outline: &OutlineId,
    section: &SectionId,
    under: Option<SectionId>,
    after: Option<SectionId>,
) -> Result<OutlineChange, ApiError> {
    let payload = MoveSectionRequest {
        under: under.map(|under| under.to_string()),
        after: after.map(|after| after.to_string()),
    };
    let response = sent_with(
        Request::put(&format!("{OUTLINES}/{outline}/sections/{section}/place")),
        &payload,
    )
    .await?;

    changed(response).await
}

pub async fn promote(outline: &OutlineId, section: &SectionId) -> Result<OutlineChange, ApiError> {
    urged(outline, section, "promote").await
}

pub async fn demote(outline: &OutlineId, section: &SectionId) -> Result<OutlineChange, ApiError> {
    urged(outline, section, "demote").await
}

pub async fn remove(outline: &OutlineId, section: &SectionId) -> Result<OutlineChange, ApiError> {
    let response = sent(Request::delete(&format!(
        "{OUTLINES}/{outline}/sections/{section}"
    )))
    .await?;

    changed(response).await
}

pub async fn attach(
    outline: &OutlineId,
    attachment: &Attachment,
    to: &SectionId,
    after: Option<Attachment>,
) -> Result<OutlineChange, ApiError> {
    let payload = AttachRequest {
        attachment: to_dto(attachment),
        section: to.to_string(),
        after: after.as_ref().map(to_dto),
    };
    let response = sent_with(
        Request::post(&format!("{OUTLINES}/{outline}/attachments")),
        &payload,
    )
    .await?;

    changed(response).await
}

pub async fn detach(outline: &OutlineId, attachment: &Attachment) -> Result<(), ApiError> {
    let (kind, id) = match attachment {
        Attachment::Scene(scene) => ("scenes", scene.to_string()),
        Attachment::Idea(idea) => ("ideas", idea.to_string()),
    };
    let response = sent(Request::delete(&format!(
        "{OUTLINES}/{outline}/attachments/{kind}/{id}"
    )))
    .await?;

    checked(answered(response).await?, SUBJECT)
        .await
        .map(|_| ())
}

const TOLD_AS_THEY_ARE: [&str; 2] = [refused::TITLE_TOO_LONG, refused::TITLE_CONTROL_CHARACTER];

async fn changed(response: Response) -> Result<OutlineChange, ApiError> {
    Ok(as_change(parsed(answered(response).await?, SUBJECT).await?))
}

async fn answered(response: Response) -> Result<Response, ApiError> {
    if !matches!(response.status(), 404 | 422) {
        return Ok(response);
    }

    match rejection(response).await {
        ApiError::Rejected(refusal)
            if refusal
                .kind
                .as_deref()
                .is_some_and(|kind| TOLD_AS_THEY_ARE.contains(&kind)) =>
        {
            Err(ApiError::Rejected(refusal))
        }
        _ => Err(ApiError::Overtaken),
    }
}

async fn urged(
    outline: &OutlineId,
    section: &SectionId,
    how: &str,
) -> Result<OutlineChange, ApiError> {
    let response = sent(Request::post(&format!(
        "{OUTLINES}/{outline}/sections/{section}/{how}"
    )))
    .await?;

    changed(response).await
}

fn to_dto(attachment: &Attachment) -> AttachmentDTO {
    match attachment {
        Attachment::Scene(scene) => AttachmentDTO::Scene {
            id: scene.to_string(),
        },
        Attachment::Idea(idea) => AttachmentDTO::Idea {
            id: idea.to_string(),
        },
    }
}

fn as_attachment(dto: AttachmentDTO) -> Attachment {
    match dto {
        AttachmentDTO::Scene { id } => Attachment::Scene(SceneId::from(id)),
        AttachmentDTO::Idea { id } => Attachment::Idea(IdeaId::from(id)),
    }
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
        attachments: dto.attachments.into_iter().map(as_attachment).collect(),
    }
}

fn as_change(dto: OutlineChangeDTO) -> OutlineChange {
    OutlineChange {
        id: OutlineId::from(dto.id),
        version: dto.version,
        sections: dto.sections.into_iter().map(as_section_change).collect(),
    }
}

fn as_section_change(dto: ChangedSectionDTO) -> SectionChange {
    SectionChange {
        id: SectionId::from(dto.section),
        parent: dto.parent.map(SectionId::from),
        title: dto.title,
        attachments: dto
            .attachments
            .map(|attachments| attachments.into_iter().map(as_attachment).collect()),
    }
}
