use std::path::PathBuf;

use super::Settings;
use crate::ServedClient;

pub const WEAVELING_CLIENT_DIR: &str = "WEAVELING_CLIENT_DIR";
pub(super) const LEPTOS_SITE_ROOT: &str = "LEPTOS_SITE_ROOT";
pub(super) const LEPTOS_WATCH: &str = "LEPTOS_WATCH";
pub(super) const LEPTOS_RELOAD_PORT: &str = "LEPTOS_RELOAD_PORT";

impl Settings {
    /// The built client to serve next to `/api`, if one is named.
    /// It is named by the setting, or else by cargo-leptos, which starts this server with the site it built.
    pub fn client(&self) -> Option<ServedClient> {
        let dir = self.client_dir.as_ref()?;

        Some(ServedClient {
            dir: PathBuf::from(dir.value.trim()),
            changes_told_at: self.reload_port,
        })
    }
}
