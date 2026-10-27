mod accounts;
mod app;
mod appearances;
mod boards;
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
mod nearness;
mod outline;
mod problem;
mod projects;
mod quiet_note;
mod refusals;
mod route;
mod scenes;
mod setup;
mod shell;
mod speech;
mod statistics;
mod theme;
mod tray;
mod writing;

use leptos::prelude::*;

use crate::app::App;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
