mod app;
mod boards;
mod http;
mod outline;
mod passages;
mod pieces;
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
