use eventsourcing::Agent;
use feedback_contract::{
    AboutDTO, AskedDTO, CommentBlockDTO, CommentRunDTO, FeedbackCountDTO, FeedbackDTO, PendingDTO,
    PlaceDTO, ReadingCopyPlaceDTO, SubjectDTO,
};
use feedback_core::{
    About, Asked, BlockPath, CommentBlock, CommentRun, Counted, Feedback, Pending, Places,
    ReadingCopyLink, ReadingCopyPlace, SceneLink, SectionLink, Subject,
};

pub fn to_pending_dto(pending: &Pending) -> PendingDTO {
    let counted = |count: &usize| u32::try_from(*count).unwrap_or(u32::MAX);

    PendingDTO {
        scenes: pending
            .scenes
            .iter()
            .map(|(scene, count)| (scene.to_string(), counted(count)))
            .collect(),
        sections: pending
            .sections
            .iter()
            .map(|(section, count)| (section.to_string(), counted(count)))
            .collect(),
        total: counted(&pending.total),
    }
}

pub fn to_count_dto(counted: &Counted) -> FeedbackCountDTO {
    let Subject::ReadingCopy(reading_copy) = &counted.subject;

    FeedbackCountDTO {
        id: reading_copy.to_string(),
        not_done: u32::try_from(counted.not_done).unwrap_or(u32::MAX),
        your_drafts: u32::try_from(counted.your_drafts).unwrap_or(u32::MAX),
    }
}

pub fn to_dto(feedback: &Feedback, asking: &Agent) -> FeedbackDTO {
    FeedbackDTO {
        id: feedback.id.to_string(),
        about: to_about_dto(&feedback.about),
        comment: feedback
            .comment
            .blocks()
            .iter()
            .map(to_comment_block_dto)
            .collect(),
        draft: feedback.is_draft(),
        by_you: &feedback.written_by == asking,
        // TODO: the writer's name, as the feedback feature keeps them from the accounts (step 5)
        written_by: None,
        written_at: clock::text::serialize(feedback.written_at),
        submitted_at: feedback.submitted_at.map(clock::text::serialize),
        edited: feedback.edited,
        done: feedback.done,
    }
}

fn to_about_dto(about: &About) -> AboutDTO {
    match about {
        About::ReadingCopy {
            reading_copy,
            place,
        } => AboutDTO::ReadingCopy {
            reading_copy: reading_copy.to_string(),
            place: to_reading_copy_place_dto(place),
        },
    }
}

fn to_reading_copy_place_dto(place: &ReadingCopyPlace) -> ReadingCopyPlaceDTO {
    match place {
        ReadingCopyPlace::Passage {
            scene,
            block_path,
            from,
            to,
            quote,
        } => ReadingCopyPlaceDTO::Passage {
            scene: scene.to_string(),
            block_path: block_path.indices().to_vec(),
            from: *from,
            to: *to,
            quote: quote.clone(),
        },
        ReadingCopyPlace::Scene(scene) => ReadingCopyPlaceDTO::Scene {
            scene: scene.to_string(),
        },
        ReadingCopyPlace::Section(section) => ReadingCopyPlaceDTO::Section {
            section: section.to_string(),
        },
        ReadingCopyPlace::Whole => ReadingCopyPlaceDTO::Whole,
    }
}

pub fn about_of(about: AboutDTO) -> About {
    match about {
        AboutDTO::ReadingCopy {
            reading_copy,
            place,
        } => About::ReadingCopy {
            reading_copy: ReadingCopyLink::from(reading_copy),
            place: reading_copy_place_of(place),
        },
    }
}

fn reading_copy_place_of(place: ReadingCopyPlaceDTO) -> ReadingCopyPlace {
    match place {
        ReadingCopyPlaceDTO::Passage {
            scene,
            block_path,
            from,
            to,
            quote,
        } => ReadingCopyPlace::Passage {
            scene: SceneLink::from(scene),
            block_path: BlockPath::new(block_path),
            from,
            to,
            quote,
        },
        ReadingCopyPlaceDTO::Scene { scene } => ReadingCopyPlace::Scene(SceneLink::from(scene)),
        ReadingCopyPlaceDTO::Section { section } => {
            ReadingCopyPlace::Section(SectionLink::from(section))
        }
        ReadingCopyPlaceDTO::Whole => ReadingCopyPlace::Whole,
    }
}

fn to_comment_block_dto(block: &CommentBlock) -> CommentBlockDTO {
    match block {
        CommentBlock::Paragraph(runs) => CommentBlockDTO::Paragraph {
            runs: runs.iter().map(to_comment_run_dto).collect(),
        },
        CommentBlock::List { ordered, items } => CommentBlockDTO::List {
            ordered: *ordered,
            items: items
                .iter()
                .map(|item| item.iter().map(to_comment_run_dto).collect())
                .collect(),
        },
    }
}

fn to_comment_run_dto(run: &CommentRun) -> CommentRunDTO {
    match run {
        CommentRun::Text {
            text,
            emphasis,
            strong,
            link,
        } => CommentRunDTO::Text {
            text: text.clone(),
            emphasis: *emphasis,
            strong: *strong,
            link: link.clone(),
        },
        CommentRun::Break => CommentRunDTO::Break,
    }
}

pub fn comment_of(blocks: Vec<CommentBlockDTO>) -> Vec<CommentBlock> {
    blocks
        .into_iter()
        .map(|block| match block {
            CommentBlockDTO::Paragraph { runs } => {
                CommentBlock::Paragraph(runs.into_iter().map(comment_run_of).collect())
            }
            CommentBlockDTO::List { ordered, items } => CommentBlock::List {
                ordered,
                items: items
                    .into_iter()
                    .map(|item| item.into_iter().map(comment_run_of).collect())
                    .collect(),
            },
        })
        .collect()
}

fn comment_run_of(run: CommentRunDTO) -> CommentRun {
    match run {
        CommentRunDTO::Text {
            text,
            emphasis,
            strong,
            link,
        } => CommentRun::Text {
            text,
            emphasis,
            strong,
            link,
        },
        CommentRunDTO::Break => CommentRun::Break,
    }
}

/// Which feedback is asked for, as the core tells it: a list left empty does not narrow.
pub fn asked_of(asked: AskedDTO) -> Asked {
    let mut scenes = Vec::new();
    let mut sections = Vec::new();
    for place in &asked.places {
        match place {
            PlaceDTO::Scene(scene) => scenes.push(SceneLink::from(scene.as_str())),
            PlaceDTO::Section(section) => sections.push(SectionLink::from(section.as_str())),
        }
    }

    Asked {
        project: asked.project,
        subjects: (!asked.subjects.is_empty())
            .then(|| asked.subjects.into_iter().map(subject_of).collect()),
        places: (!asked.places.is_empty()).then_some(Places::Text { scenes, sections }),
    }
}

pub fn subject_of(subject: SubjectDTO) -> Subject {
    match subject {
        SubjectDTO::ReadingCopy(copy) => Subject::ReadingCopy(ReadingCopyLink::from(copy.as_str())),
    }
}
