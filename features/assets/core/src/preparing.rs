use tokio::sync::Semaphore;

/// How many uploads are prepared at once.
/// Each one holds a thread and the whole image in memory.
/// A photo of fifty million pixels takes about 200 MB.
pub const PREPARED_AT_ONCE: usize = 2;

static PREPARING: Semaphore = Semaphore::const_new(PREPARED_AT_ONCE);

/// Prepares an upload outside the async runtime, so a request waiting on it does not hold a worker.
/// At most `PREPARED_AT_ONCE` uploads are prepared at a time.
/// So a flood of uploads waits its turn instead of taking every thread and all the memory.
pub(crate) async fn aside<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let _turn = PREPARING
        .acquire()
        .await
        .expect("the preparing's turns are never closed");

    tokio::task::spawn_blocking(work)
        .await
        .expect("preparing never panics")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn no_more_uploads_are_prepared_at_once_than_their_turns() {
        // given: three times as many uploads prepared at once as may be
        let running = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        let asked: Vec<_> = (0..PREPARED_AT_ONCE * 3)
            .map(|_| {
                let (running, most) = (running.clone(), most.clone());
                tokio::spawn(aside(move || {
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    most.fetch_max(now, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(30));
                    running.fetch_sub(1, Ordering::SeqCst);
                }))
            })
            .collect();

        // when: they all run
        for prepared in asked {
            prepared.await.expect("preparing never panics");
        }

        // then: no more ran at once than there are turns
        assert!(most.load(Ordering::SeqCst) <= PREPARED_AT_ONCE);
    }
}
