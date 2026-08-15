mod api;
mod app;
mod confirm_delete;
mod new_project;
mod overlays;
mod project_row;
mod workspace;

use leptos::prelude::*;

use crate::app::App;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
