use leptos::prelude::*;
use leptos::task;
use scenes_contract::SceneSummaryDTO;

use crate::http::ApiError;
use crate::live::Following;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneSummary;
use crate::scenes::service;
use live_contract::FrameDTO;

pub fn follow(
    project: &ProjectId,
    listed: impl Fn(&[SceneSummary]) + Copy + 'static,
    told: impl Fn(&SceneSummary) + Copy + 'static,
    failed: impl Fn(ApiError) + Copy + 'static,
) -> (Following, impl Fn() + Clone + 'static) {
    let meanwhile = StoredValue::new(Vec::<SceneSummary>::new());
    let asking = {
        let project = project.clone();

        move || {
            let project = project.clone();
            meanwhile.set_value(Vec::new());

            task::spawn_local(async move {
                match service::in_project(&project).await {
                    Ok(all) => {
                        listed(&all);
                        for summary in meanwhile.try_get_value().unwrap_or_default() {
                            told(&summary);
                        }
                    }
                    Err(failure) => failed(failure),
                }
            });
        }
    };
    let following = Following::start(
        &format!("/api/scenes/live?project={project}"),
        move |frame: FrameDTO| {
            if let Ok(found) = serde_json::from_value::<SceneSummaryDTO>(frame.event) {
                let summary = SceneSummary::from(found);
                _ = meanwhile.try_update_value(|held| held.push(summary.clone()));
                told(&summary);
            }
        },
        asking.clone(),
    );

    (following, asking)
}
