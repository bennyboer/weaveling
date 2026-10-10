mod app;
mod appearances;
mod boards;
mod http;
mod icons;
mod ideas;
mod inputs;
mod moment;
mod outline;
mod projects;
mod refusals;
mod route;
mod scenes;
mod shell;
mod theme;
mod tray;

use leptos::prelude::*;

use crate::app::App;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
