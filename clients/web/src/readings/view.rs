use std::collections::HashSet;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use live_contract::FrameDTO;
use readings_contract::BookChangeDTO;
use web_sys::{ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::asking::Asking;
use crate::assets::service as assets;
use crate::drawer;
use crate::drawer::{Drawer, Edge, Inside};
use crate::feedback::column as feedback;
use crate::feedback::column::Commenting;
use crate::feedback::model::ReadingCopyPlace;
use crate::http::ApiError;
use crate::icons;
use crate::icons::Icon;
use crate::live::Subscription;
use crate::outline::model::SectionId;
use crate::problem;
use crate::projects::model::ProjectId;
use crate::readings::comment_on_live;
use crate::readings::model::{Applied, Block, Book, Change, Marks, Run};
use crate::readings::passage;
use crate::readings::reading_copies::{ListedReadingCopy, ReadingCopyId};
use crate::readings::service;
use crate::readings::switcher;
use crate::route;
use crate::scene_break;
use crate::scenes::model::SceneId;
use crate::speech;

const FIGURE_WIDTH: u32 = 1536;
const CONTENTS: &str = "reading-contents";
/// Where whether a copy's feedback was left open is kept.
pub(crate) const FEEDBACK_KEPT: &str = "weaveling.reading-feedback";
/// Where the contents lie over the text, as on a phone, they start closed.
const LYING_OVER_THE_TEXT: &str = "(width <= 60rem)";
/// Where the contents and a copy's feedback do not both fit beside the text, the feedback starts closed.
const ROOM_FOR_BOTH: &str = "(width > 82.5rem)";

/// What the book shows, in reading order: each section, and each scene with something written.
/// A scene after another scene stands behind a scene break.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Shown {
    Section {
        section: SectionId,
        depth: u8,
        title: String,
    },
    Scene {
        scene: SceneId,
        after_a_scene: bool,
    },
}

/// The work to read: the live book, or a reading copy of it.
/// Above the text, the switcher chooses between them.
pub fn reading_view(
    slug: String,
    copy: Option<String>,
    copy_gone: RwSignal<bool>,
) -> impl IntoView {
    let beside_the_text = window()
        .match_media(LYING_OVER_THE_TEXT)
        .ok()
        .flatten()
        .is_none_or(|query| !query.matches());
    let room_for_both = window()
        .match_media(ROOM_FOR_BOTH)
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    let rail = Drawer::as_last_left("weaveling.reading-contents", beside_the_text);
    let said = Drawer::as_last_left(FEEDBACK_KEPT, room_for_both);
    if copy.is_some() {
        drawer::keep_apart(rail, said);
    }
    let project = route::project_id(&slug);
    let book = RwSignal::new(None::<Book>);
    let failed = RwSignal::new(None::<ApiError>);
    let chosen = RwSignal::new(None::<ListedReadingCopy>);
    let copy = copy.map(ReadingCopyId::from);
    match &copy {
        None => following_live(project.clone(), book, failed),
        Some(copy) => reading_a_copy(copy.clone(), book, chosen, failed, copy_gone),
    }
    let copies = LocalResource::new({
        let project = project.clone();

        move || {
            let project = project.clone();

            async move { service::copies(&project).await.unwrap_or_default() }
        }
    });
    let has_book = Memo::new(move |_| book.with(Option::is_some));
    let commenting = match &copy {
        Some(copy) => Commenting::in_copy(copy.clone(), said),
        None => Commenting::on_the_live_work(),
    };

    move || {
        if let Some(failure) = failed.get() {
            Some(problem::problem(failure).into_any())
        } else if has_book.get() {
            let bar = switcher::reading_bar(slug.clone(), copy.clone(), copies, chosen);
            let feedback = copy.clone().map(|copy| {
                (
                    feedback::on_reading_copy(copy, commenting, said, book),
                    said,
                )
            });
            let on_the_live_work = copy
                .is_none()
                .then(|| comment_on_live::asked(slug.clone(), commenting, book));

            Some(
                (
                    read(book, rail, bar.into_any(), feedback, Some(commenting)),
                    on_the_live_work,
                )
                    .into_any(),
            )
        } else {
            None
        }
    }
}

/// Follows the live book as it is written: each change told swaps in the piece that changed.
fn following_live(
    project: ProjectId,
    book: RwSignal<Option<Book>>,
    failed: RwSignal<Option<ApiError>>,
) {
    let asking = Asking::new();
    let reading_now = StoredValue::new(false);
    let reading = Action::new_local(move |project: &ProjectId| {
        let project = project.clone();
        let asked = asking.next();
        reading_now.set_value(true);

        async move {
            let read = service::live_book(&project).await;
            if asking.is_latest(asked) {
                reading_now.set_value(false);
                match read {
                    Ok(read) => {
                        failed.set(None);
                        book.update(|held| {
                            let read = match held.as_ref() {
                                Some(before) => read.keeping_unchanged(before),
                                None => read,
                            };
                            *held = Some(read);
                        });
                    }
                    Err(failure) => failed.set(Some(failure)),
                }
            }
        }
    });
    let heard = {
        let project = project.clone();

        move |told: FrameDTO| {
            let Ok(change) = serde_json::from_value::<BookChangeDTO>(told.event) else {
                return;
            };
            let applied = book
                .try_update(|held| held.as_mut().map(|held| held.apply(Change::from(change))))
                .flatten();
            if reading_now.get_value() || applied != Some(Applied::Whole) {
                reading.dispatch(project.clone());
            }
        }
    };
    let opened = {
        let project = project.clone();

        move || _ = reading.dispatch(project.clone())
    };
    let following = StoredValue::new_local(Some(Subscription::start(
        "readings",
        &[("project", project.to_string().as_str())],
        heard,
        opened,
    )));
    on_cleanup(move || following.set_value(None));
}

/// Reads a reading copy once, as it never changes; a copy not there is told apart, so the page can say so as a whole.
fn reading_a_copy(
    copy: ReadingCopyId,
    book: RwSignal<Option<Book>>,
    chosen: RwSignal<Option<ListedReadingCopy>>,
    failed: RwSignal<Option<ApiError>>,
    gone: RwSignal<bool>,
) {
    leptos::task::spawn_local(async move {
        match service::reading_copy(&copy).await {
            Ok((listed, read)) => {
                _ = chosen.try_set(Some(listed));
                _ = book.try_set(Some(read));
            }
            Err(ApiError::NotFound(_)) => _ = gone.try_set(true),
            Err(failure) => _ = failed.try_set(Some(failure)),
        }
    });
}

/// The text between the contents on the left and, in a copy, its feedback on the right.
fn read(
    book: RwSignal<Option<Book>>,
    rail: Drawer,
    bar: AnyView,
    feedback: Option<(impl IntoView + 'static, Drawer)>,
    commenting: Option<Commenting>,
) -> impl IntoView {
    let title = Memo::new(move |_| {
        book.with(|held| held.as_ref().map(|held| held.title.clone()))
            .unwrap_or_default()
    });

    let (feedback, said) = match feedback {
        Some((column, said)) => (Some(column), Some(said)),
        None => (None, None),
    };

    (
        contents(book, rail, commenting),
        html::div().class("reading-view").child((
            bar,
            html::article().class("book").child((
                html::h1().child(move || title.get()),
                parts(book, commenting),
                commenting.map(passage::offered),
            )),
        )),
        feedback,
        toggles(rail, said),
    )
}

/// On a phone the contents, and a copy's feedback, open from the status bar, as no tab may lie over the text.
fn toggles(rail: Drawer, said: Option<Drawer>) -> impl IntoView {
    html::div().class("drawer-toggles").child((
        toggle(rail, "contents", CONTENTS, Icon::Section),
        said.map(|said| toggle(said, "feedback", feedback::FEEDBACK, Icon::Comment)),
    ))
}

fn toggle(
    drawer: Drawer,
    named: &'static str,
    controls: &'static str,
    icon: Icon,
) -> impl IntoView {
    html::button()
        .r#type("button")
        .attr("aria-label", speech::said(named))
        .attr("title", speech::said(named))
        .attr("aria-controls", controls)
        .attr("aria-expanded", move || drawer.is_open().to_string())
        .on(ev::click, move |_| drawer.toggle())
        .child(icons::mark(icon))
}

/// A section in the contents: how deep it lies, its title, whether sections lie under it, and whether those are folded away;
/// in a copy, how many comments in it are not done yet, those under it too while it is folded.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Listed {
    section: SectionId,
    depth: u8,
    title: String,
    nests: bool,
    folded: bool,
    not_done: usize,
}

/// The contents, as in the writing view: the sections only, each folding away the sections under it, and all of them folded or unfolded at once;
/// in a copy, each counts the comments in it not done yet.
fn contents(
    book: RwSignal<Option<Book>>,
    rail: Drawer,
    commenting: Option<Commenting>,
) -> impl IntoView {
    let folded = RwSignal::new(HashSet::<SectionId>::new());
    let nesting = Memo::new(move |_| {
        book.with(|held| {
            held.iter()
                .flat_map(|held| {
                    held.sections
                        .windows(2)
                        .filter(|pair| pair[1].depth > pair[0].depth)
                        .map(|pair| pair[0].section.clone())
                })
                .collect::<HashSet<_>>()
        })
    });
    let not_done = Memo::new(move |_| {
        commenting
            .zip(book.get())
            .map(|(commenting, book)| commenting.not_done_by_section(&book))
            .unwrap_or_default()
    });
    let listed = Memo::new(move |_| {
        let nesting = nesting.get();
        not_done.with(|not_done| {
            folded.with(|folded| {
                book.with(|held| {
                    let mut listed: Vec<Listed> = Vec::new();
                    let mut folded_below: Option<u8> = None;
                    for outlined in held.iter().flat_map(|held| &held.sections) {
                        let counted = not_done.get(&outlined.section).copied().unwrap_or(0);
                        if folded_below.is_some_and(|depth| outlined.depth > depth) {
                            if let Some(shut) = listed.last_mut() {
                                shut.not_done += counted;
                            }
                            continue;
                        }
                        let nests = nesting.contains(&outlined.section);
                        let shut = nests && folded.contains(&outlined.section);
                        folded_below = shut.then_some(outlined.depth);
                        listed.push(Listed {
                            section: outlined.section.clone(),
                            depth: outlined.depth,
                            title: outlined.title.clone(),
                            nests,
                            folded: shut,
                            not_done: counted,
                        });
                    }

                    listed
                })
            })
        })
    });
    let all_unfolded = move || folded.with(HashSet::is_empty);
    let all_folded = move || {
        let nesting = nesting.get();
        folded.with(|folded| nesting.iter().all(|section| folded.contains(section)))
    };

    drawer::drawer(
        rail,
        Edge::Left,
        || speech::say("contents"),
        Inside {
            id: CONTENTS,
            named: speech::said("contents").into(),
            called: "contents",
            head: (
                html::p().class("tally").child(speech::said("contents")),
                html::button()
                    .r#type("button")
                    .class("fold-all")
                    .attr("aria-label", speech::said("contents-unfold-all"))
                    .attr("title", speech::said("contents-unfold-all"))
                    .disabled(all_unfolded)
                    .on(ev::click, move |_| folded.set(HashSet::new()))
                    .child(icons::mark(Icon::UnfoldAll)),
                html::button()
                    .r#type("button")
                    .class("fold-all")
                    .attr("aria-label", speech::said("contents-fold-all"))
                    .attr("title", speech::said("contents-fold-all"))
                    .disabled(all_folded)
                    .on(ev::click, move |_| folded.set(nesting.get_untracked()))
                    .child(icons::mark(Icon::FoldAll)),
            ),
            body: html::ol()
                .class("rail-entries")
                .child(For(ForProps::builder()
                    .each(move || listed.get())
                    .key(Clone::clone)
                    .children(move |listed| entry(listed, folded, rail))
                    .build())),
        },
    )
}

fn entry(listed: Listed, folded: RwSignal<HashSet<SectionId>>, rail: Drawer) -> impl IntoView {
    let shown = match listed.title.trim() {
        "" => speech::say("heading-untitled"),
        _ => listed.title.clone(),
    };
    let turning = listed.section.clone();
    let going = listed.section.clone();

    html::li()
        .class("rail-section")
        .style(("--depth", listed.depth.to_string()))
        .child((
            listed.nests.then(|| {
                let unfolded = !listed.folded;

                html::button()
                    .r#type("button")
                    .class("fold")
                    .attr("aria-expanded", unfolded.to_string())
                    .attr(
                        "aria-label",
                        if unfolded {
                            speech::say_about("outline-fold", "section", &shown)
                        } else {
                            speech::say_about("contents-unfold", "section", &shown)
                        },
                    )
                    .on(ev::click, move |_| {
                        folded.update(|folded| {
                            if !folded.remove(&turning) {
                                folded.insert(turning.clone());
                            }
                        });
                    })
                    .child(icons::mark(if unfolded {
                        Icon::Open
                    } else {
                        Icon::Closed
                    }))
            }),
            html::button()
                .r#type("button")
                .class("rail-entry")
                .on(ev::click, move |_| go_to(&going, rail))
                .child((
                    html::span().class("title").child(shown),
                    (listed.not_done > 0).then(|| {
                        let mut args = FluentArgs::new();
                        args.set("count", listed.not_done);

                        html::span()
                            .class("not-done")
                            .attr("aria-label", speech::say_with("contents-not-done", &args))
                            .attr("title", speech::say_with("contents-not-done", &args))
                            .child(listed.not_done.to_string())
                    }),
                )),
        ))
}

/// Brings a section's heading to the top, and puts the contents away where they lie over the text.
fn go_to(section: &SectionId, rail: Drawer) {
    if let Some(heading) = document().get_element_by_id(&heading_id(section)) {
        let at_the_top = ScrollIntoViewOptions::new();
        at_the_top.set_block(ScrollLogicalPosition::Start);
        heading.scroll_into_view_with_scroll_into_view_options(&at_the_top);
    }
    let narrow = window()
        .match_media(LYING_OVER_THE_TEXT)
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    if narrow {
        rail.close();
    }
}

pub(crate) fn heading_id(section: &SectionId) -> String {
    format!("section-{section}")
}

/// Whether the drawers lie over the text, as on a phone.
pub(crate) fn lying_over_the_text() -> bool {
    window()
        .match_media(LYING_OVER_THE_TEXT)
        .ok()
        .flatten()
        .is_some_and(|query| query.matches())
}

fn shown(book: &Book) -> Vec<Shown> {
    let mut shown = Vec::new();
    for outlined in &book.sections {
        let mut after_a_scene = false;
        shown.push(Shown::Section {
            section: outlined.section.clone(),
            depth: outlined.depth,
            title: outlined.title.clone(),
        });
        for scene in &outlined.scenes {
            if book.text_of(scene).is_some() {
                shown.push(Shown::Scene {
                    scene: scene.clone(),
                    after_a_scene,
                });
                after_a_scene = true;
            }
        }
    }

    shown
}

/// The book's parts in reading order; in a copy, each section and scene offers to be commented on.
fn parts(book: RwSignal<Option<Book>>, commenting: Option<Commenting>) -> impl IntoView {
    let shown = Memo::new(move |_| book.with(|held| held.as_ref().map(shown).unwrap_or_default()));

    For(ForProps::builder()
        .each(move || shown.get())
        .key(Clone::clone)
        .children(move |part| match part {
            Shown::Section {
                section,
                depth,
                title,
            } => {
                let heading = section_heading(&section, depth, title.clone());
                let commented = ReadingCopyPlace::Section(section.clone());

                html::div()
                    .class("section-head")
                    .child((
                        heading,
                        commenting.map(move |commenting| {
                            comment_here(
                                speech::say_about("feedback-comment-on-section", "section", &title),
                                commenting,
                                commented,
                            )
                        }),
                    ))
                    .into_any()
            }
            Shown::Scene {
                scene,
                after_a_scene,
            } => {
                let text = {
                    let scene = scene.clone();

                    Memo::new(move |_| {
                        book.with(|held| held.as_ref().and_then(|held| held.text_of(&scene)))
                    })
                };

                (
                    after_a_scene.then(scene_break),
                    html::section()
                        .class("scene")
                        .attr("data-scene", scene.to_string())
                        .child((
                            commenting.map(|commenting| {
                                comment_here(
                                    speech::say("feedback-comment-on-this-scene"),
                                    commenting,
                                    ReadingCopyPlace::Scene(scene.clone()),
                                )
                            }),
                            move || {
                                text.get()
                                    .map(|told| blocks_of(told.blocks().to_vec(), &[]))
                            },
                        )),
                )
                    .into_any()
            }
        })
        .build())
}

/// A quiet button beside a section or a scene that begins a comment on it,
/// shown at all times and filled while you have a draft about it, since such a draft is not marked in the text;
/// then it leads to that draft instead.
fn comment_here(named: String, commenting: Commenting, place: ReadingCopyPlace) -> impl IntoView {
    let drafted = {
        let place = place.clone();

        move || commenting.drafted_on(&place)
    };

    html::button()
        .r#type("button")
        .class("comment-here")
        .class(("drafted", drafted))
        .attr("aria-label", named.clone())
        .attr("title", named)
        .on(ev::click, move |_| commenting.begin(place.clone()))
        .child(icons::mark(Icon::Comment))
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

/// The blocks of a scene, or of a quote or a list item in it, each block that holds text marked with its path from the scene,
/// so a passage selected in it can say where it lies.
fn blocks_of(blocks: Vec<Block>, path: &[u32]) -> Vec<AnyView> {
    blocks
        .into_iter()
        .enumerate()
        .map(|(nth, held)| {
            let mut at = path.to_vec();
            at.push(u32::try_from(nth).unwrap_or(u32::MAX));
            block(held, at)
        })
        .collect()
}

pub(crate) fn path_written(path: &[u32]) -> String {
    path.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn block(block: Block, path: Vec<u32>) -> AnyView {
    let written = path_written(&path);
    match block {
        Block::Paragraph(runs) => html::p()
            .attr("data-block", written)
            .child(runs_of(runs))
            .into_any(),
        Block::Heading { level, runs } => match level {
            1 => html::h4()
                .attr("data-block", written)
                .child(runs_of(runs))
                .into_any(),
            2 => html::h5()
                .attr("data-block", written)
                .child(runs_of(runs))
                .into_any(),
            _ => html::h6()
                .attr("data-block", written)
                .child(runs_of(runs))
                .into_any(),
        },
        Block::Quote(blocks) => html::blockquote()
            .child(blocks_of(blocks, &path))
            .into_any(),
        Block::List { ordered, items } => {
            let items = items
                .into_iter()
                .enumerate()
                .map(|(nth, item)| {
                    let mut at = path.clone();
                    at.push(u32::try_from(nth).unwrap_or(u32::MAX));
                    html::li().child(blocks_of(item, &at))
                })
                .collect::<Vec<_>>();

            if ordered {
                html::ol().child(items).into_any()
            } else {
                html::ul().child(items).into_any()
            }
        }
        Block::Code(text) => html::pre()
            .attr("data-block", written)
            .child(html::code().child(text))
            .into_any(),
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

pub(crate) fn safe_link(link: &str) -> bool {
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
