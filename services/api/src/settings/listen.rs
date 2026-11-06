use std::net::SocketAddr;

use thiserror::Error;

use super::{Setting, Settings};

pub const WEAVELING_LISTEN: &str = "WEAVELING_LISTEN";
const LISTEN_UNLESS_SET: &str = "127.0.0.1:3000";

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{setting}; set it to an address and a port, such as {LISTEN_UNLESS_SET}")]
pub struct Unlistenable {
    pub setting: Setting,
}

impl Settings {
    pub fn listen(&self) -> Result<SocketAddr, Unlistenable> {
        let Some(setting) = &self.listen else {
            return Ok(LISTEN_UNLESS_SET
                .parse()
                .expect("the built-in place to listen is an address and a port"));
        };

        setting.value.trim().parse().map_err(|_| Unlistenable {
            setting: setting.clone(),
        })
    }
}
