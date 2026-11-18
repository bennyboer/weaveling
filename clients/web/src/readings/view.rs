use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use web_sys::{ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::assets::service as assets;
use crate::drawer;
use crate::drawer::{Drawer, Edge, Inside};
use crate::outline::model::SectionId;
use crate::problem;
use crate::readings::model::{Block, Book, Marks, Part, Run};
use crate::readings::service;
use crate::route;
use crate::scene_break;
use crate::speech;

const FIGURE_WIDTH: u32 = 1536;
const CONTENTS: &str = "reading-contents";

pub fn reading_view(project: String) -> impl IntoView {
    let rail = Drawer::as_last_left("weaveling.reading-contents", true);
    let project = route::project_id(&project);
    let book = LocalResource::new(move || {
        let project = project.clone();

        async move { service::live_book(&project).await }
    });

    move || {
        book.get().map(|loaded| match loaded {
            Ok(book) => read(book, rail).into_any(),
            Err(failure) => problem::problem(failure).into_any(),
        })
    }
}

fn read(book: Book, rail: Drawer) -> impl IntoView {
    (
        contents(&book.parts, rail),
        html::div().class("reading-view").child(
            html::article()
                .class("book")
                .child((html::h1().child(book.title), parts(book.parts))),
        ),
    )
}

fn contents(parts: &[Part], rail: Drawer) -> impl IntoView + use<> {
    let sections = parts
        .iter()
        .filter_map(|part| match part {
            Part::Section {
                section,
                depth,
                title,
            } => Some(entry(section.clone(), *depth, title.clone())),
            Part::Scene { .. } => None,
        })
        .collect::<Vec<_>>();

    drawer::drawer(
        rail,
        Edge::Left,
        || speech::say("contents"),
        Inside {
            id: CONTENTS,
            named: speech::said("contents").into(),
            called: "contents",
            head: html::p().class("tally").child(speech::said("contents")),
            body: html::ol().class("rail-entries").child(sections),
        },
    )
}

fn entry(section: SectionId, depth: u8, title: String) -> impl IntoView {
    html::li()
        .class("rail-section")
        .style(("--depth", depth.to_string()))
        .child(
            html::button()
                .r#type("button")
                .class("rail-entry")
                .on(ev::click, move |_| go_to(&section))
                .child(title),
        )
}

fn go_to(section: &SectionId) {
    if let Some(heading) = document().get_element_by_id(&heading_id(section)) {
        let at_the_top = ScrollIntoViewOptions::new();
        at_the_top.set_block(ScrollLogicalPosition::Start);
        heading.scroll_into_view_with_scroll_into_view_options(&at_the_top);
    }
}

fn heading_id(section: &SectionId) -> String {
    format!("section-{section}")
}

fn parts(parts: Vec<Part>) -> impl IntoView {
    let mut after_a_scene = false;

    parts
        .into_iter()
        .map(|part| match part {
            Part::Section {
                section,
                depth,
                title,
            } => {
                after_a_scene = false;

                section_heading(&section, depth, title).into_any()
            }
            Part::Scene { scene, blocks } => {
                let between = after_a_scene.then(scene_break);
                after_a_scene = true;

                (
                    between,
                    html::section()
                        .class("scene")
                        .attr("data-scene", scene.to_string())
                        .child(blocks_of(blocks)),
                )
                    .into_any()
            }
        })
        .collect::<Vec<_>>()
}

fn section_heading(section: &SectionId, depth: u8, title: String) -> impl IntoView {
    let id = heading_id(section);

    match depth {
        0 => html::h2().id(id).child(title).into_any(),
        1 => html::h3().id(id).child(title).into_any(),
        _ => html::h4().id(id).child(title).into_any(),
    }
}

fn scene_break() -> impl IntoView {
    html::div()
        .class("scene-break")
        .role("separator")
        .child(scene_break::diamonds())
}

fn blocks_of(blocks: Vec<Block>) -> Vec<AnyView> {
    blocks.into_iter().map(block).collect()
}

fn block(block: Block) -> AnyView {
    match block {
        Block::Paragraph(runs) => html::p().child(runs_of(runs)).into_any(),
        Block::Heading { level, runs } => match level {
            1 => html::h4().child(runs_of(runs)).into_any(),
            2 => html::h5().child(runs_of(runs)).into_any(),
            _ => html::h6().child(runs_of(runs)).into_any(),
        },
        Block::Quote(blocks) => html::blockquote().child(blocks_of(blocks)).into_any(),
        Block::List { ordered, items } => {
            let items = items
                .into_iter()
                .map(|item| html::li().child(blocks_of(item)))
                .collect::<Vec<_>>();

            if ordered {
                html::ol().child(items).into_any()
            } else {
                html::ul().child(items).into_any()
            }
        }
        Block::Code(text) => html::pre().child(html::code().child(text)).into_any(),
        Block::Rule => scene_break().into_any(),
        Block::Figure {
            asset,
            size,
            caption,
        } => html::figure()
            .class(format!("figure {}", size.name()))
            .child((
                html::img()
                    .src(assets::content(&asset, FIGURE_WIDTH))
                    .alt(caption.clone()),
                (!caption.is_empty()).then(|| html::figcaption().child(caption)),
            ))
            .into_any(),
    }
}

fn runs_of(runs: Vec<Run>) -> Vec<AnyView> {
    runs.into_iter().map(run).collect()
}

fn run(run: Run) -> AnyView {
    match run {
        Run::Text { text, marks } => marked(text, marks),
        Run::Break => html::br().into_any(),
    }
}

fn marked(text: String, marks: Marks) -> AnyView {
    let mut shown = text.into_any();
    if marks.code {
        shown = html::code().child(shown).into_any();
    }
    if marks.emphasis {
        shown = html::em().child(shown).into_any();
    }
    if marks.strong {
        shown = html::strong().child(shown).into_any();
    }
    match marks.link.as_deref().filter(|link| safe_link(link)) {
        Some(link) => html::a()
            .href(link.to_owned())
            .target("_blank")
            .rel("noopener noreferrer")
            .child(shown)
            .into_any(),
        None => shown,
    }
}

fn safe_link(link: &str) -> bool {
    let lowered = link.to_ascii_lowercase();

    ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| lowered.starts_with(scheme) && lowered.len() > scheme.len())
        && !link.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_and_mail_links_are_followed() {
        // expect: web and mail addresses are links
        assert!(safe_link("https://example.org/loom"));
        assert!(safe_link("HTTP://example.org"));
        assert!(safe_link("mailto:ada@example.org"));
        // and: anything else, or a scheme alone, or one with spaces, is not
        assert!(!safe_link("javascript:alert(1)"));
        assert!(!safe_link("https://"));
        assert!(!safe_link("https://example.org/a b"));
        assert!(!safe_link("/projects/loom"));
    }
}
