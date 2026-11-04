use leptos::prelude::*;
use leptos::task;
use scenes_contract::SceneSummaryDTO;

use crate::asking::Asking;
use crate::http::ApiError;
use crate::live::Subscription;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneSummary;
use crate::scenes::service;
use live_contract::FrameDTO;

pub fn follow(
    project: &ProjectId,
    listed: impl Fn(&[SceneSummary]) + Copy + 'static,
    told: impl Fn(&SceneSummary) + Copy + 'static,
    failed: impl Fn(ApiError) + Copy + 'static,
) -> (Subscription, impl Fn() + Clone + 'static) {
    let meanwhile = StoredValue::new(None::<Vec<SceneSummary>>);
    let asking = Asking::new();
    let asking = {
        let project = project.clone();

        move || {
            let project = project.clone();
            let asked = asking.next();
            meanwhile.set_value(Some(Vec::new()));

            task::spawn_local(async move {
                let answer = service::in_project(&project).await;
                if !asking.is_latest(asked) {
                    return;
                }
                match answer {
                    Ok(all) => {
                        listed(&all);
                        for summary in meanwhile.try_get_value().flatten().unwrap_or_default() {
                            told(&summary);
                        }
                    }
                    Err(failure) => failed(failure),
                }
                _ = meanwhile.try_set_value(None);
            });
        }
    };
    let following = Subscription::start(
        "scenes",
        &[("project", project.to_string().as_str())],
        move |frame: FrameDTO| {
            if let Ok(found) = serde_json::from_value::<SceneSummaryDTO>(frame.event) {
                let summary = SceneSummary::from(found);
                _ = meanwhile.try_update_value(|held| {
                    if let Some(held) = held {
                        held.push(summary.clone());
                    }
                });
                told(&summary);
            }
        },
        asking.clone(),
    );

    (following, asking)
}
