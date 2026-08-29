mod app;
mod http;
mod passages;
mod pieces;
mod projects;
mod route;

use leptos::prelude::*;

use crate::app::App;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
