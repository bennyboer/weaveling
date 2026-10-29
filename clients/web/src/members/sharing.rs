use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html, view};
use leptos_router::components::A;

use crate::accounts::{CurrentSession, SignInState};
use crate::dialog;
use crate::icons;
use crate::icons::Icon;
use crate::invitations::invited::Invited;
use crate::invitations::listing;
use crate::invitations::service::InvitedInto;
use crate::members::people;
use crate::members::people::People;
use crate::route;
use crate::speech;
use crate::speech::Speech;

pub fn share_in_masthead(project: String, named: Signal<String>) -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let signed_in = Memo::new(move |_| matches!(session.state(), SignInState::SignedIn(_)));

    move || signed_in.get().then(|| share(project.clone(), named))
}

fn share(project: String, named: Signal<String>) -> impl IntoView {
    let open = RwSignal::new(false);
    let close = move || open.set(false);

    (
        html::button()
            .r#type("button")
            .class("share-button")
            .attr("aria-label", speech::said("share"))
            .attr("title", speech::said("share"))
            .attr("aria-haspopup", "dialog")
            .on(ev::click, move |_| open.set(true))
            .child((
                icons::mark(Icon::Share),
                html::span()
                    .class("share-word")
                    .attr("aria-hidden", "true")
                    .child(speech::said("share")),
            )),
        move || open.get().then(|| sharing(project.clone(), named, close)),
    )
}

fn sharing(
    project: String,
    named: Signal<String>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let speech = expect_context::<Speech>();
    let id = route::project_id(&project);
    let people = People::asked(id.clone());
    let invited = Invited::unasked(InvitedInto::Project(id.to_string()));
    let owning = Memo::new(move |_| people.owning());
    Effect::new(move |_| {
        if owning.get() {
            invited.load(None);
        }
    });
    let titled = Signal::derive(move || {
        let mut told = FluentArgs::new();
        told.set("project", named.get());
        speech.say_with("share-title", &told)
    });

    dialog::modal(
        titled,
        "share",
        close,
        html::div().class("operating sharing").child((
            move || owning.get().then(|| listing::invite_form(invited)),
            html::section().child((
                html::h3().child(speech::said("share-people")),
                people::people(people),
            )),
            move || {
                owning.get().then(|| {
                    html::section().child((
                        html::h3().child(speech::said("share-invited")),
                        listing::invited_list(invited),
                    ))
                })
            },
            view! {
                <A href=route::members(&project) attr:class="all-members">
                    {speech::said("share-all-members")}
                </A>
            },
        )),
    )
}
