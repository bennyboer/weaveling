use tokio::sync::Semaphore;

/// How many Argon2 hashes run at the same time.
/// Each one uses a thread and 19 MiB for tens of milliseconds.
pub const HASHES_AT_ONCE: usize = 4;

static HASHING: Semaphore = Semaphore::const_new(HASHES_AT_ONCE);

/// Runs hashing work outside the async runtime, at most `HASHES_AT_ONCE` at a time.
/// A request waiting for it does not block a worker.
/// A flood of hashing work waits its turn, instead of taking every thread and all the memory.
pub(crate) async fn aside<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let _turn = HASHING
        .acquire()
        .await
        .expect("the hashing's turns are never closed");

    tokio::task::spawn_blocking(work)
        .await
        .expect("hashing never panics")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn no_more_hashes_run_at_once_than_their_turns() {
        // given: twice as many hashes asked for at once as may run
        let running = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        let asked: Vec<_> = (0..HASHES_AT_ONCE * 2)
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
        for hashed in asked {
            hashed.await.expect("hashing never panics");
        }

        // then: no more ran at once than there are turns
        assert!(most.load(Ordering::SeqCst) <= HASHES_AT_ONCE);
    }
}
