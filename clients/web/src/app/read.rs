use leptos::IntoView;
use leptos::prelude::*;
use leptos_router::hooks;

use crate::app::missing;
use crate::app::project_page;
use crate::readings::view;
use crate::shell::Viewing;
use crate::speech;

/// The reading view; or, once the reading copy asked for is found not to be there, a page saying so,
/// as for a project that is not there.
#[component]
pub fn ReadPage() -> impl IntoView {
    let params = hooks::use_params_map();
    let copy_gone = RwSignal::new(false);
    let asked = Memo::new(move |_| params.read().get("copy"));
    Effect::new(move |_| {
        asked.track();
        copy_gone.set(false);
    });

    move || match copy_gone.get() {
        true => missing::not_found(|| speech::say_about("problem-not-found", "subject", "copy"))
            .into_any(),
        false => project_page::project_page(Viewing::Read, move |project| {
            view::reading_view(project, params.read().get("copy"), copy_gone)
        })
        .into_any(),
    }
}
