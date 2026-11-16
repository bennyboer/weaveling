use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html, view};
use leptos_router::components::A;

use crate::assets::service::{Holder, Purpose, Use};
use crate::projects::model::ProjectId;
use crate::route;
use crate::scenes::model::{SceneId, SceneSummary};
use crate::scenes::service as scenes;
use crate::speech;

/// Lists what uses an asset, each a link to where it is used.
/// A scene is named by its title, which is asked for only when the list is shown.
/// A scene that cannot be read, or is not found, is named without a link.
/// Given `followed`, a scene's link is followed by it rather than by the browser,
/// so a view that shows the scene already can go there itself.
pub fn listed(
    id: String,
    project: Option<String>,
    uses: Vec<Use>,
    followed: Option<Callback<SceneId>>,
) -> impl IntoView {
    let scenes_known = RwSignal::new(None::<Vec<SceneSummary>>);
    let in_scenes = uses.iter().any(|used| matches!(used.by, Holder::Scene(_)));
    if let (true, Some(project)) = (in_scenes, project.clone()) {
        spawn_local(async move {
            let known = scenes::in_project(&ProjectId::from(project))
                .await
                .unwrap_or_default();
            scenes_known.try_set(Some(known));
        });
    }

    html::ul().class("uses").child(
        uses.into_iter()
            .map(|used| {
                html::li().child(use_shown(
                    used,
                    id.clone(),
                    project.clone(),
                    scenes_known,
                    followed,
                ))
            })
            .collect::<Vec<_>>(),
    )
}

fn use_shown(
    used: Use,
    id: String,
    project: Option<String>,
    scenes_known: RwSignal<Option<Vec<SceneSummary>>>,
    followed: Option<Callback<SceneId>>,
) -> impl IntoView {
    match used.by {
        Holder::Account(_) if used.purpose == Purpose::AccountPicture => view! {
            <A href=route::ACCOUNT>{speech::said("storage-use-picture")}</A>
        }
        .into_any(),
        Holder::Account(_) => html::span()
            .child(speech::said("storage-use-other"))
            .into_any(),
        Holder::Scene(scene) => (move || {
            scene_used(
                &scene,
                &id,
                project.as_deref(),
                scenes_known.get(),
                followed,
            )
        })
        .into_any(),
    }
}

fn scene_used(
    scene: &SceneId,
    id: &str,
    project: Option<&str>,
    known: Option<Vec<SceneSummary>>,
    followed: Option<Callback<SceneId>>,
) -> AnyView {
    let Some(known) = known else {
        return html::span()
            .class("hint")
            .child(speech::said("storage-use-finding"))
            .into_any();
    };
    let named = known
        .iter()
        .find(|summary| &summary.id == scene)
        .map(SceneSummary::shown_as);

    match (named, project, followed) {
        (Some(named), Some(project), Some(followed)) => {
            let scene = scene.clone();
            html::a()
                .href(route::writing_at_asset(project, &scene, id))
                .on(ev::click, move |event| {
                    event.prevent_default();
                    followed.run(scene.clone());
                })
                .child(named)
                .into_any()
        }
        (Some(named), Some(project), None) => {
            let at = route::writing_at_asset(project, scene, id);
            view! { <A href=at>{named}</A> }.into_any()
        }
        _ => html::span()
            .child(speech::said("storage-use-scene-closed"))
            .into_any(),
    }
}
