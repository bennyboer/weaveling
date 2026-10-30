use std::fmt;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast::{self, Receiver, Sender, error::RecvError};

const BACKLOG: usize = 256;

pub trait Channel: Clone + PartialEq + Send + Sync + 'static {}

impl<T: Clone + PartialEq + Send + Sync + 'static> Channel for T {}

#[derive(Clone)]
struct Told<C> {
    channel: C,
    frame: Arc<str>,
}

pub struct Broadcaster<C> {
    told: Sender<Told<C>>,
}

pub struct Hearing<C> {
    channel: C,
    told: Receiver<Told<C>>,
}

#[async_trait]
pub trait Heard: Send {
    async fn next(&mut self) -> Option<Arc<str>>;
}

impl<C: Channel> Broadcaster<C> {
    pub fn new() -> Self {
        Self::holding(BACKLOG)
    }

    pub(crate) fn holding(backlog: usize) -> Self {
        Self {
            told: broadcast::channel(backlog).0,
        }
    }

    #[cfg(test)]
    pub(crate) fn listener_count(&self) -> usize {
        self.told.receiver_count()
    }

    pub fn tell(&self, channel: C, frame: String) {
        _ = self.told.send(Told {
            channel,
            frame: Arc::from(frame),
        });
    }

    pub fn hear(&self, channel: C) -> Hearing<C> {
        Hearing {
            channel,
            told: self.told.subscribe(),
        }
    }
}

impl<C> Clone for Broadcaster<C> {
    fn clone(&self) -> Self {
        Self {
            told: self.told.clone(),
        }
    }
}

impl<C> Debug for Broadcaster<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Broadcaster").finish_non_exhaustive()
    }
}

impl<C: Channel> Default for Broadcaster<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<C: Channel> Heard for Hearing<C> {
    async fn next(&mut self) -> Option<Arc<str>> {
        loop {
            match self.told.recv().await {
                Ok(told) if told.channel == self.channel => return Some(told.frame),
                Ok(_) => {}
                Err(RecvError::Lagged(_) | RecvError::Closed) => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time;

    use super::*;

    async fn soon(hearing: &mut Hearing<&'static str>) -> Option<Arc<str>> {
        time::timeout(Duration::from_millis(100), hearing.next())
            .await
            .ok()
            .flatten()
    }

    #[tokio::test]
    async fn a_hearer_hears_only_what_is_told_about_its_own() {
        let broadcaster = Broadcaster::new();
        let mut mine = broadcaster.hear("outline_1");

        broadcaster.tell("outline_2", "theirs".to_owned());
        broadcaster.tell("outline_1", "mine".to_owned());

        assert_eq!(soon(&mut mine).await.as_deref(), Some("mine"));
    }

    #[tokio::test]
    async fn everyone_hearing_the_same_hears_it() {
        let broadcaster = Broadcaster::new();
        let mut one = broadcaster.hear("outline_1");
        let mut other = broadcaster.hear("outline_1");

        broadcaster.tell("outline_1", "moved".to_owned());

        assert_eq!(soon(&mut one).await.as_deref(), Some("moved"));
        assert_eq!(soon(&mut other).await.as_deref(), Some("moved"));
    }

    #[tokio::test]
    async fn what_was_told_before_anyone_listened_is_gone() {
        let broadcaster = Broadcaster::new();
        broadcaster.tell("outline_1", "early".to_owned());
        let mut late = broadcaster.hear("outline_1");

        assert_eq!(
            soon(&mut late).await,
            None,
            "a live channel says what happens now; catching up is a reload's job"
        );
    }

    #[tokio::test]
    async fn a_hearer_that_fell_behind_stops_hearing() {
        let broadcaster = Broadcaster::holding(2);
        let mut slow = broadcaster.hear("outline_1");

        for nth in 0..3 {
            broadcaster.tell("outline_1", nth.to_string());
        }

        assert_eq!(
            slow.next().await,
            None,
            "frames it missed cannot be told again, so it ends and its client reloads"
        );
    }
}
