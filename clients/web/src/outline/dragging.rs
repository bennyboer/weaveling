use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::icons;
use crate::icons::Icon;
use crate::outline::model::{Attachment, SectionId};
use crate::outline::tree;
use crate::outline::tree::Held;
use crate::speech;

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Landing {
    Before(SectionId),
    Into(SectionId),
    After(SectionId),
}

pub(super) fn grip(section: SectionId, held: Held) -> impl IntoView {
    let state = held.state;
    let labelled = section.clone();
    let hauled = section.clone();

    html::button()
        .r#type("button")
        .class("grip")
        .attr("aria-label", move || {
            tree::about_section("outline-move", &state.title_of(&labelled))
        })
        .on(ev::pointerdown, move |event| {
            event.stop_propagation();

            if let Some(under) = event
                .current_target()
                .and_then(|it| it.dyn_into::<HtmlElement>().ok())
            {
                let _ = under.set_pointer_capture(event.pointer_id());
            }

            held.hauling.set(Some(hauled.clone()));
            held.landing.set(None);
        })
        .on(ev::pointermove, move |event| {
            let Some(section) = held.hauling.get_untracked() else {
                return;
            };

            held.landing.set(landing_at(
                event.client_x(),
                event.client_y(),
                &section,
                held,
            ));
        })
        .on(ev::pointerup, move |_| {
            let section = held.hauling.get_untracked();
            let at = held.landing.get_untracked();
            held.hauling.set(None);
            held.landing.set(None);

            let (Some(section), Some(at)) = (section, at) else {
                return;
            };

            let (under, after) = match at {
                Landing::Into(target) => {
                    let last = state.landing_into(&target);
                    (Some(target), last)
                }
                Landing::Before(target) => {
                    (state.parent_of(&target), state.landing_before(&target))
                }
                Landing::After(target) => (state.parent_of(&target), Some(target)),
            };

            state.place(section, under, after);
        })
        .on(ev::pointercancel, move |_| {
            held.hauling.set(None);
            held.landing.set(None);
        })
        .child(icons::mark(Icon::Grip))
}

fn landing_at(x: i32, y: i32, section: &SectionId, held: Held) -> Option<Landing> {
    let row = document()
        .element_from_point(x as f32, y as f32)?
        .closest("[data-section]")
        .ok()
        .flatten()?;
    let target = SectionId::from(row.get_attribute("data-section")?);

    if held.state.would_swallow(section, &target) {
        return None;
    }

    let edge = row.get_bounding_client_rect();
    let into = (f64::from(y) - edge.top()) / edge.height();

    Some(match into {
        near if near < 0.28 => Landing::Before(target),
        near if near > 0.72 => Landing::After(target),
        _ => Landing::Into(target),
    })
}

pub(super) fn carried(attachment: Attachment, shown: String, held: Held) -> impl IntoView {
    let mine = attachment.clone();
    let taken = attachment.clone();
    let dropped = attachment;
    let marked = icons::mark(tree::icon_for(&mine));

    html::li().child(
        html::button()
            .r#type("button")
            .prop("disabled", move || !held.may_write.get())
            .class(("carrying", move || {
                held.carrying
                    .with(|attachment| attachment.as_ref() == Some(&mine))
            }))
            .attr("aria-label", {
                let shown = shown.clone();

                move || speech::say_about("outline-place", "name", &shown)
            })
            .on(ev::pointerdown, move |event| {
                if let Some(under) = event
                    .current_target()
                    .and_then(|it| it.dyn_into::<HtmlElement>().ok())
                {
                    let _ = under.set_pointer_capture(event.pointer_id());
                }

                held.carrying.set(Some(taken.clone()));
                held.over.set(None);
            })
            .on(ev::pointermove, move |event| {
                if held.carrying.with_untracked(Option::is_some) {
                    held.over
                        .set(section_under(event.client_x(), event.client_y()));
                }
            })
            .on(ev::pointerup, move |_| {
                let Some(landing) = held.over.get_untracked() else {
                    return;
                };

                held.state.attach(dropped.clone(), landing);
                held.carrying.set(None);
                held.over.set(None);
            })
            .child((marked, shown)),
    )
}

fn section_under(x: i32, y: i32) -> Option<SectionId> {
    document()
        .element_from_point(x as f32, y as f32)?
        .closest("[data-section]")
        .ok()
        .flatten()
        .and_then(|row| row.get_attribute("data-section"))
        .map(SectionId::from)
}
