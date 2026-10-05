mod app;
mod boards;
mod http;
mod icons;
mod ideas;
mod outline;
mod passages;
mod projects;
mod route;
mod shell;
mod theme;
mod tray;

use leptos::prelude::*;

use crate::app::App;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
