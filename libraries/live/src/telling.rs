use std::sync::atomic::{AtomicU64, Ordering};

use live_contract::FrameDTO;
use serde::Serialize;

use crate::Broadcaster;

pub struct Teller {
    broadcaster: Broadcaster,
    told: AtomicU64,
}

impl Teller {
    pub fn new(broadcaster: Broadcaster) -> Self {
        Self {
            broadcaster,
            told: AtomicU64::new(0),
        }
    }

    pub fn tell(&self, about: &str, of: &str, event: impl Serialize) {
        let version = self.told.fetch_add(1, Ordering::AcqRel) + 1;

        self.broadcaster.tell(
            about,
            serde_json::to_string(&FrameDTO {
                of: Some(of.to_owned()),
                version,
                at: None,
                event: serde_json::to_value(event)
                    .expect("what is told is plain data and cannot fail to serialize"),
            })
            .expect("a frame is plain data and cannot fail to serialize"),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::{Value, json};
    use tokio::time;

    use super::*;

    #[tokio::test]
    async fn each_telling_says_what_it_is_of_and_is_newer_than_the_last() {
        let broadcaster = Broadcaster::new();
        let mut hearing = broadcaster.hear("project_1");
        let teller = Teller::new(broadcaster);

        teller.tell("project_1", "scene_1", json!({ "words": 42 }));
        teller.tell("project_1", "scene_2", json!({ "words": 7 }));

        let mut told = Vec::new();
        for _ in 0..2 {
            let frame = time::timeout(Duration::from_millis(100), hearing.next())
                .await
                .expect("told at once")
                .expect("the broadcaster still tells");
            told.push(serde_json::from_str::<Value>(&frame).expect("a frame is JSON"));
        }

        assert_eq!(
            told,
            vec![
                json!({ "of": "scene_1", "version": 1, "event": { "words": 42 } }),
                json!({ "of": "scene_2", "version": 2, "event": { "words": 7 } }),
            ]
        );
    }
}
