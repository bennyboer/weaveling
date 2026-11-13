mod accounts;
mod act;
mod app;
mod appearances;
mod asking;
mod avatar;
mod boards;
mod copy_button;
mod dialog;
mod drawer;
mod http;
mod icons;
mod ideas;
mod instance;
mod invitations;
mod kept;
mod live;
mod members;
mod moment;
mod more_menu;
mod nearness;
mod notifications;
mod outline;
mod path;
mod problem;
mod projects;
mod quiet_note;
mod refusals;
mod route;
mod scenes;
mod setup;
mod shell;
mod size;
mod speech;
mod statistics;
mod theme;
mod tray;
mod working_copy;
mod writing;

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::app::App;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        crate::speech::readied().await;
        mount_to_body(App);
    });
}
