use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::{Notify, watch};

/// The server's own end: told to close, it lets every socket staying open know, and waits until the last has gone.
pub struct Shutdown {
    closing: watch::Sender<bool>,
    open: Arc<Open>,
}

/// What a socket holds to hear that the server is closing, and to be waited for until it has.
#[derive(Clone)]
pub struct Closing {
    closed: watch::Receiver<bool>,
    open: Arc<Open>,
}

/// A socket still open; the server waits for it until it is dropped.
pub struct Staying {
    open: Arc<Open>,
}

#[derive(Default)]
struct Open {
    count: AtomicUsize,
    none_left: Notify,
}

impl Shutdown {
    pub fn new() -> Self {
        Self {
            closing: watch::channel(false).0,
            open: Arc::default(),
        }
    }

    pub fn closing(&self) -> Closing {
        Closing {
            closed: self.closing.subscribe(),
            open: self.open.clone(),
        }
    }

    pub fn close(&self) {
        self.closing.send_replace(true);
    }

    pub async fn drained(&self) {
        loop {
            let none_left = self.open.none_left.notified();
            if self.open.count.load(Ordering::Acquire) == 0 {
                return;
            }
            none_left.await;
        }
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Closing {
    pub fn stay(&self) -> Staying {
        self.open.count.fetch_add(1, Ordering::AcqRel);

        Staying {
            open: self.open.clone(),
        }
    }

    pub async fn closed(&self) {
        let mut closed = self.closed.clone();
        _ = closed.wait_for(|closed| *closed).await;
    }
}

impl Drop for Staying {
    fn drop(&mut self) {
        if self.open.count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.open.none_left.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn whoever_stays_hears_the_server_close() {
        let shutdown = Shutdown::new();
        let closing = shutdown.closing();

        let before = tokio::time::timeout(Duration::from_millis(50), closing.closed()).await;
        shutdown.close();
        let after = tokio::time::timeout(Duration::from_millis(50), closing.closed()).await;

        assert!(before.is_err(), "nothing closes before it is told to");
        assert!(after.is_ok());
    }

    #[tokio::test]
    async fn the_server_waits_until_the_last_socket_has_gone() {
        let shutdown = Shutdown::new();
        let first = shutdown.closing().stay();
        let second = shutdown.closing().stay();

        drop(first);
        let waiting = tokio::time::timeout(Duration::from_millis(50), shutdown.drained()).await;
        drop(second);
        let drained = tokio::time::timeout(Duration::from_millis(50), shutdown.drained()).await;

        assert!(waiting.is_err(), "one socket is still open");
        assert!(drained.is_ok());
    }

    #[tokio::test]
    async fn nothing_open_is_drained_at_once() {
        let shutdown = Shutdown::new();
        let _router_holds_one = shutdown.closing();

        assert!(
            tokio::time::timeout(Duration::from_millis(50), shutdown.drained())
                .await
                .is_ok(),
            "holding what tells of closing is not staying open"
        );
    }
}
