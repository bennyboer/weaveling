use leptos::IntoView;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use leptos_router::NavigateOptions;
use leptos_router::hooks;

use crate::assets::choosing;
use crate::assets::service::AssetType;
use crate::outline::model::SectionId;
use crate::route;
use crate::scenes::model::SceneId;
use crate::writing::beginnings::CarriedImage;
use crate::writing::side::Side;
use crate::writing::write_view_state::WriteViewState;

/// Where an image was asked for: at a place in a scene's text, which the ticket stands for,
/// or on the line a new scene begins on, in a section after a scene or first in it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Asked {
    InScene(SceneId, u32),
    Beginning(SectionId, Option<SceneId>),
}

/// Where an image was asked for. While it is set, the dialog for choosing an image is open.
#[derive(Clone, Copy)]
pub struct ImageChoice {
    asked: RwSignal<Option<Asked>>,
}

impl ImageChoice {
    pub fn nothing() -> Self {
        Self {
            asked: RwSignal::new(None),
        }
    }

    /// Opens the dialog for an image to go to the place the ticket stands for.
    pub fn ask(self, scene: SceneId, ticket: u32) {
        self.asked.set(Some(Asked::InScene(scene, ticket)));
    }

    /// Opens the dialog for an image to begin a new scene with, on the line that begins it.
    pub fn ask_for_beginning(self, section: SectionId, after: Option<SceneId>) {
        self.asked.set(Some(Asked::Beginning(section, after)));
    }

    fn closed(self) {
        self.asked.set(None);
    }
}

/// The dialog for choosing an image to put into a scene, shown while a scene asks for one.
/// The image chosen is the project's own, copied into it by the dialog if it was kept elsewhere.
pub fn dialog(state: WriteViewState) -> impl IntoView {
    move || {
        state.image_choice.asked.get().map(|asked| match asked {
            Asked::InScene(scene, ticket) => chooser(scene, ticket, state).into_any(),
            Asked::Beginning(section, after) => beginning(section, after, state).into_any(),
        })
    }
}

fn chooser(scene: SceneId, ticket: u32, state: WriteViewState) -> impl IntoView {
    let scene = StoredValue::new(scene);

    let closed = move || {
        state.image_choice.closed();
        state.editors.told(&scene.get_value(), |editor| {
            editor.forget_waiting(ticket);
            editor.focus();
        });
    };
    let chosen = move |asset: String| {
        state.image_choice.closed();
        state.editors.told(&scene.get_value(), |editor| {
            editor.insert_figure(&asset, ticket);
            editor.forget_waiting(ticket);
            editor.focus();
        });
    };

    let navigate = StoredValue::new_local(hooks::use_navigate());
    let followed = move |used_in: SceneId, asset: String| {
        state
            .editors
            .told(&scene.get_value(), |editor| editor.forget_waiting(ticket));
        follow(state, navigate, used_in, asset);
    };

    choosing::dialog(
        state.structure.project().to_string(),
        AssetType::Image,
        chosen,
        closed,
        followed,
    )
}

/// The dialog for an image asked for on the line a new scene begins on.
/// The image chosen begins the scene, as typing there would, and goes into it once its editor is there.
fn beginning(section: SectionId, after: Option<SceneId>, state: WriteViewState) -> impl IntoView {
    let begun = StoredValue::new((section, after));

    let closed = move || state.image_choice.closed();
    let chosen = move |asset: String| {
        state.image_choice.closed();
        let (section, after) = begun.get_value();
        state
            .beginnings
            .opening_image_given(section.clone(), CarriedImage::Asset(asset));
        state.beginnings.begin_scene(section, after);
    };
    let navigate = StoredValue::new_local(hooks::use_navigate());
    let followed = move |used_in: SceneId, asset: String| follow(state, navigate, used_in, asset);

    choosing::dialog(
        state.structure.project().to_string(),
        AssetType::Image,
        chosen,
        closed,
        followed,
    )
}

/// Leaves the dialog for a scene that uses an asset, with that asset chosen in it.
/// A scene this view shows is landed in where it is. Any other is opened on the other side, in the book or among the loose scenes.
fn follow(
    state: WriteViewState,
    navigate: StoredValue<impl Fn(&str, NavigateOptions) + 'static, LocalStorage>,
    used_in: SceneId,
    asset: String,
) {
    state.image_choice.closed();
    let shown_here = match state.side {
        Side::Book => state.structure.holds(&used_in),
        Side::Loose => state.loose.in_order_untracked().contains(&used_in),
    };
    if shown_here {
        if let Some(followed) = document()
            .active_element()
            .and_then(|focused| focused.dyn_into::<HtmlElement>().ok())
        {
            _ = followed.blur();
        }
        state.left_off.land(Some(used_in), Some(asset));
    } else {
        let project = state.structure.project().to_string();
        let elsewhere = match state.side {
            Side::Book => route::loose_scene_at_asset(&project, &used_in, &asset),
            Side::Loose => route::writing_at_asset(&project, &used_in, &asset),
        };
        navigate.with_value(|navigate| navigate(&elsewhere, NavigateOptions::default()));
    }
}
