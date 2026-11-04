use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tokio_util::task::task_tracker::TaskTrackerToken;

/// The server's own end: put into the routes as an extension, every socket's `Ends` hears it close, and it waits until the last of them has gone.
#[derive(Clone, Default)]
pub struct Shutdown {
    closing: CancellationToken,
    open: TaskTracker,
}

impl Shutdown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn close(&self) {
        self.open.close();
        self.closing.cancel();
    }

    /// Returns once the server is closed and no socket's `Ends` is left.
    pub async fn drained(&self) {
        self.open.wait().await;
    }

    pub(crate) fn closing(&self) -> CancellationToken {
        self.closing.clone()
    }

    pub(crate) fn stay(&self) -> TaskTrackerToken {
        self.open.token()
    }
}
