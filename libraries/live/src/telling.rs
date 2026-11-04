use std::sync::Mutex;

use live_contract::FrameDTO;
use serde::Serialize;

use crate::{Broadcaster, Channel};

/// Tells frames it numbers itself, for changes that carry no published version.
pub struct Teller<C> {
    broadcaster: Broadcaster<C>,
    told: Mutex<u64>,
    reading: tokio::sync::Mutex<()>,
}

impl<C: Channel> Teller<C> {
    pub fn new(broadcaster: Broadcaster<C>) -> Self {
        Self {
            broadcaster,
            told: Mutex::new(0),
            reading: tokio::sync::Mutex::new(()),
        }
    }

    pub fn tell(&self, channel: C, of: &str, event: impl Serialize) {
        let mut told = self.told.lock().expect("the count is never poisoned");
        *told += 1;
        let version = *told;

        self.broadcaster.tell(
            channel,
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

    /// Tells what `read` finds, reading one at a time, so a state read before a newer change is told before it rather than last; nothing when it finds nothing.
    pub async fn tell_latest<E: Serialize>(
        &self,
        channel: C,
        of: &str,
        read: impl Future<Output = Option<E>>,
    ) {
        let _reading = self.reading.lock().await;
        if let Some(event) = read.await {
            self.tell(channel, of, event);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use serde_json::{Value, json};
    use tokio::time;

    use super::*;
    use crate::Frames;

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

    #[tokio::test(flavor = "multi_thread")]
    async fn what_is_told_last_is_never_older_than_what_was_told_before() {
        let broadcaster = Broadcaster::new();
        let mut hearing = broadcaster.hear("project_1");
        let teller = Arc::new(Teller::new(broadcaster));
        let kept = Arc::new(AtomicU64::new(1));

        let slow = tokio::spawn({
            let (teller, kept) = (teller.clone(), kept.clone());
            async move {
                teller
                    .tell_latest("project_1", "scene_1", async {
                        let read = kept.load(Ordering::SeqCst);
                        time::sleep(Duration::from_millis(50)).await;
                        Some(json!({ "title": read }))
                    })
                    .await;
            }
        });
        time::sleep(Duration::from_millis(10)).await;
        kept.store(2, Ordering::SeqCst);
        teller
            .tell_latest("project_1", "scene_1", async {
                Some(json!({ "title": kept.load(Ordering::SeqCst) }))
            })
            .await;
        slow.await.expect("the slow telling finishes");

        let mut told = Vec::new();
        for _ in 0..2 {
            let frame = time::timeout(Duration::from_millis(100), hearing.next())
                .await
                .expect("told at once")
                .expect("the broadcaster still tells");
            let frame = serde_json::from_str::<Value>(&frame).expect("a frame is JSON");
            told.push(json!([frame["version"], frame["event"]["title"]]));
        }

        assert_eq!(
            told,
            vec![json!([1, 1]), json!([2, 2])],
            "a reading begun before a newer change is told before the reading of it, never after"
        );
    }
}
