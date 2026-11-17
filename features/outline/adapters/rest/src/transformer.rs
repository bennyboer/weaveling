use eventsourcing::Version;
use outline_contract::{
    AttachmentDTO, ChangedSectionDTO, OutlineChangeDTO, OutlineDTO, PlacedSectionDTO,
};
use outline_core::{Attachment, Changed, Outline, PlacedSection};

pub(crate) fn to_change_dto(outline: &str, changed: &Changed) -> OutlineChangeDTO {
    let standing = &changed.standing;
    let nothing_else_landed = standing.version == changed.version;

    OutlineChangeDTO {
        id: outline.to_owned(),
        since: changed.since.count(),
        version: standing.version.count(),
        project: standing.state.project().to_string(),
        sections: standing
            .state
            .sections()
            .iter()
            .map(|placed| ChangedSectionDTO {
                section: placed.section.to_string(),
                parent: placed.parent.map(|parent| parent.to_string()),
                title: placed.title.to_string(),
                attachments: (!nothing_else_landed
                    || changed.attachments_changed_in.contains(&placed.section))
                .then(|| placed.attachments.iter().map(to_attachment_dto).collect()),
            })
            .collect(),
    }
}

pub(crate) fn to_dto(outline: &str, held: &Outline, version: Version) -> OutlineDTO {
    OutlineDTO {
        id: outline.to_owned(),
        version: version.count(),
        project: held.project().to_string(),
        sections: held.sections().iter().map(to_section_dto).collect(),
    }
}

fn to_attachment_dto(attachment: &Attachment) -> AttachmentDTO {
    match attachment {
        Attachment::Scene(scene) => AttachmentDTO::Scene {
            id: scene.to_string(),
        },
        Attachment::Idea(idea) => AttachmentDTO::Idea {
            id: idea.to_string(),
        },
    }
}

fn to_section_dto(placed: &PlacedSection) -> PlacedSectionDTO {
    PlacedSectionDTO {
        section: placed.section.to_string(),
        parent: placed.parent.map(|parent| parent.to_string()),
        title: placed.title.to_string(),
        attachments: placed.attachments.iter().map(to_attachment_dto).collect(),
    }
}
