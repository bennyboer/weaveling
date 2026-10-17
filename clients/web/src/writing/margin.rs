use leptos::prelude::*;
use web_sys::HtmlElement;

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::writing::asking::Asking;
use crate::writing::bringing_back::Landing;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Marked {
    Scene(SceneId),
    Section(SectionId),
    Gap(Landing),
    Edge(Edge),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Adding,
    Changing,
}

#[derive(Clone, Copy)]
pub struct Margin {
    open: RwSignal<Option<(Marked, Menu)>>,
    renaming: RwSignal<Option<SceneId>>,
    asking: RwSignal<Option<Asking>>,
    bringing_back: RwSignal<Option<Landing>>,
    placing: RwSignal<Option<SceneId>>,
    writing: StoredValue<Option<HtmlElement>, LocalStorage>,
}

impl Margin {
    pub fn closed() -> Self {
        Self {
            open: RwSignal::new(None),
            renaming: RwSignal::new(None),
            asking: RwSignal::new(None),
            bringing_back: RwSignal::new(None),
            placing: RwSignal::new(None),
            writing: StoredValue::new_local(None),
        }
    }

    pub fn is_open(self, marked: &Marked, menu: Menu) -> bool {
        self.open
            .with(|open| open.as_ref() == Some(&(marked.clone(), menu)))
    }

    pub fn open_from(self, marked: Marked, writing: HtmlElement) {
        self.writing.set_value(Some(writing));
        self.open.set(Some((marked, Menu::Changing)));
    }

    pub fn opened_by_keys(self) -> bool {
        self.writing.with_value(Option::is_some)
    }

    fn refocus_writing(self) {
        if let Some(writing) = self.writing.get_value() {
            self.writing.set_value(None);
            let _ = writing.focus();
        }
    }

    pub fn let_go(self) {
        self.writing.set_value(None);
        self.close();
    }

    pub fn toggle(self, marked: Marked, menu: Menu) {
        self.writing.set_value(None);
        let wanted = (marked, menu);
        let already_open = self
            .open
            .with_untracked(|open| open.as_ref() == Some(&wanted));

        self.open.set((!already_open).then_some(wanted));
    }

    pub fn close(self) {
        if self.open.with_untracked(Option::is_some) {
            self.open.set(None);
        }
    }

    pub fn is_renaming(self, scene: &SceneId) -> bool {
        self.renaming
            .with(|renaming| renaming.as_ref() == Some(scene))
    }

    pub fn start_renaming(self, scene: SceneId) {
        self.close();
        self.renaming.set(Some(scene));
    }

    pub fn stop_renaming(self, scene: &SceneId) -> bool {
        let was = self
            .renaming
            .with_untracked(|renaming| renaming.as_ref() == Some(scene));

        if was {
            self.renaming.set(None);
        }

        was
    }

    pub fn asking(self) -> Option<Asking> {
        self.asking.get()
    }

    pub fn asking_untracked(self) -> bool {
        self.asking.with_untracked(Option::is_some)
    }

    pub fn ask(self, question: Asking) {
        self.close();
        self.asking.set(Some(question));
    }

    pub fn placing(self) -> Option<SceneId> {
        self.placing.get()
    }

    pub fn place(self, scene: SceneId) {
        self.close();
        self.placing.set(Some(scene));
    }

    pub fn landing(self) -> Option<Landing> {
        self.bringing_back.get()
    }

    pub fn bring_back_to(self, landing: Landing) {
        self.close();
        self.bringing_back.set(Some(landing));
    }

    pub fn return_to_writing(self) {
        self.close();
        if self.asking.with_untracked(Option::is_some) {
            self.asking.set(None);
        }
        if self.bringing_back.with_untracked(Option::is_some) {
            self.bringing_back.set(None);
        }
        if self.placing.with_untracked(Option::is_some) {
            self.placing.set(None);
        }
        self.refocus_writing();
    }
}
