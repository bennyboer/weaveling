use gloo_net::http::{Request, RequestBuilder, Response};
use outline_contract::{
    AddSectionRequest, AddedSectionResponse, AttachRequest, AttachmentDTO, ChangedSectionDTO,
    MoveSectionRequest, OpenOutlineRequest, OutlineChangeDTO, OutlineDTO, PlacedSectionDTO,
    RetitleSectionRequest, refused,
};

use crate::http;
use crate::http::ApiError;
use crate::ideas::model::IdeaId;
use crate::outline::model::{
    Attachment, Outline, OutlineChange, OutlineId, Section, SectionChange, SectionId,
};
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;

const OUTLINES: &str = "/api/outlines";
const SUBJECT: &str = "outline";
const OPENED_IN: &str = "project";

pub async fn open(project: &ProjectId) -> Result<Outline, ApiError> {
    let payload = OpenOutlineRequest {
        project: project.to_string(),
    };
    let response = http::sent_with(Request::post(OUTLINES), &payload).await?;

    Ok(as_outline(http::parsed(response, OPENED_IN).await?))
}

pub async fn add(
    outline: &OutlineId,
    under: Option<SectionId>,
    after: Option<SectionId>,
    title: &str,
    on: Option<u64>,
) -> Result<(SectionId, OutlineChange), ApiError> {
    let payload = AddSectionRequest {
        under: under.map(|under| under.to_string()),
        after: after.map(|after| after.to_string()),
        title: title.to_owned(),
    };
    let response = http::sent_with(
        made_on(Request::post(&format!("{OUTLINES}/{outline}/sections")), on),
        &payload,
    )
    .await?;
    let added: AddedSectionResponse = http::parsed(answered(response).await?, SUBJECT).await?;

    Ok((SectionId::from(added.section), as_change(added.outline)))
}

pub async fn retitle(
    outline: &OutlineId,
    section: &SectionId,
    title: &str,
    on: Option<u64>,
) -> Result<OutlineChange, ApiError> {
    let payload = RetitleSectionRequest {
        title: title.to_owned(),
    };
    let response = http::sent_with(
        made_on(
            Request::patch(&format!("{OUTLINES}/{outline}/sections/{section}")),
            on,
        ),
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
    on: Option<u64>,
) -> Result<OutlineChange, ApiError> {
    let payload = MoveSectionRequest {
        under: under.map(|under| under.to_string()),
        after: after.map(|after| after.to_string()),
    };
    let response = http::sent_with(
        made_on(
            Request::put(&format!("{OUTLINES}/{outline}/sections/{section}/place")),
            on,
        ),
        &payload,
    )
    .await?;

    changed(response).await
}

pub async fn promote(
    outline: &OutlineId,
    section: &SectionId,
    on: Option<u64>,
) -> Result<OutlineChange, ApiError> {
    urged(outline, section, "promote", on).await
}

pub async fn demote(
    outline: &OutlineId,
    section: &SectionId,
    on: Option<u64>,
) -> Result<OutlineChange, ApiError> {
    urged(outline, section, "demote", on).await
}

pub async fn remove(
    outline: &OutlineId,
    section: &SectionId,
    on: Option<u64>,
) -> Result<OutlineChange, ApiError> {
    let response = http::sent(made_on(
        Request::delete(&format!("{OUTLINES}/{outline}/sections/{section}")),
        on,
    ))
    .await?;

    changed(response).await
}

pub async fn attach(
    outline: &OutlineId,
    attachment: &Attachment,
    to: &SectionId,
    after: Option<Attachment>,
    on: Option<u64>,
) -> Result<OutlineChange, ApiError> {
    let payload = AttachRequest {
        attachment: to_dto(attachment),
        section: to.to_string(),
        after: after.as_ref().map(to_dto),
    };
    let response = http::sent_with(
        made_on(
            Request::post(&format!("{OUTLINES}/{outline}/attachments")),
            on,
        ),
        &payload,
    )
    .await?;

    changed(response).await
}

pub async fn detach(
    outline: &OutlineId,
    attachment: &Attachment,
    on: Option<u64>,
) -> Result<(), ApiError> {
    let (kind, id) = match attachment {
        Attachment::Scene(scene) => ("scenes", scene.to_string()),
        Attachment::Idea(idea) => ("ideas", idea.to_string()),
    };
    let response = http::sent(made_on(
        Request::delete(&format!("{OUTLINES}/{outline}/attachments/{kind}/{id}")),
        on,
    ))
    .await?;

    http::checked(answered(response).await?, SUBJECT)
        .await
        .map(|_| ())
}

const TOLD_AS_THEY_ARE: [&str; 2] = [refused::TITLE_TOO_LONG, refused::TITLE_CONTROL_CHARACTER];

async fn changed(response: Response) -> Result<OutlineChange, ApiError> {
    Ok(as_change(
        http::parsed(answered(response).await?, SUBJECT).await?,
    ))
}

async fn answered(response: Response) -> Result<Response, ApiError> {
    if !matches!(response.status(), 404 | 422) {
        return Ok(response);
    }

    match http::rejection(response).await {
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
    on: Option<u64>,
) -> Result<OutlineChange, ApiError> {
    let response = http::sent(made_on(
        Request::post(&format!("{OUTLINES}/{outline}/sections/{section}/{how}")),
        on,
    ))
    .await?;

    changed(response).await
}

fn made_on(request: RequestBuilder, on: Option<u64>) -> RequestBuilder {
    match on {
        Some(version) => request.header("If-Match", &format!("\"{version}\"")),
        None => request,
    }
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
        since: dto.since,
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
