mod app;
mod boards;
mod http;
mod passages;
mod pieces;
mod projects;
mod route;
mod shell;
mod theme;

use leptos::prelude::*;

use crate::app::App;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
