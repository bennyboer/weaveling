use leptos::prelude::*;
use projects_contract::ProjectDTO;

#[derive(Clone, Copy)]
pub struct Overlays {
    open_menu: RwSignal<Option<String>>,
    confirming: RwSignal<Option<ProjectDTO>>,
}

impl Default for Overlays {
    fn default() -> Self {
        Self::new()
    }
}

impl Overlays {
    pub fn new() -> Self {
        Self {
            open_menu: RwSignal::new(None),
            confirming: RwSignal::new(None),
        }
    }

    pub fn is_menu_open(self, id: &str) -> bool {
        self.open_menu.with(|open| open.as_deref() == Some(id))
    }

    pub fn toggle_menu(self, id: String) {
        let already_open = self.is_menu_open(&id);

        self.open_menu
            .set(if already_open { None } else { Some(id) });
    }

    pub fn close_menu(self) {
        self.open_menu.set(None);
    }

    pub fn confirming(self) -> ReadSignal<Option<ProjectDTO>> {
        self.confirming.read_only()
    }

    pub fn ask_to_delete(self, project: ProjectDTO) {
        self.confirming.set(Some(project));
    }

    pub fn dismiss(self) {
        self.confirming.set(None);
    }

    pub fn close_all(self) {
        self.close_menu();
        self.dismiss();
    }
}
