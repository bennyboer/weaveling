use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::watch;
use tokio::task::JoinHandle;

#[async_trait]
pub trait Chore: Send + Sync {
    fn named(&self) -> &'static str;

    fn every(&self) -> Duration;

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>>;
}

pub struct Chores {
    stopping: watch::Sender<bool>,
    running: Vec<JoinHandle<()>>,
}

impl Chores {
    pub fn started(chores: Vec<Arc<dyn Chore>>) -> Self {
        let (stopping, stopped) = watch::channel(false);
        tracing::info!(chores = chores.len(), "started the chores");

        let running = chores
            .into_iter()
            .map(|chore| tokio::spawn(coming_round(chore, stopped.clone())))
            .collect();

        Self { stopping, running }
    }

    pub async fn stop(self) {
        let _ = self.stopping.send(true);
        for chore in self.running {
            let _ = chore.await;
        }
    }
}

async fn coming_round(chore: Arc<dyn Chore>, mut stopped: watch::Receiver<bool>) {
    loop {
        match chore.carry_out().await {
            Ok(0) => {}
            Ok(cleared) => tracing::info!(chore = chore.named(), cleared, "a chore was done"),
            Err(why) => tracing::warn!(
                chore = chore.named(),
                %why,
                "a chore failed; it is done again later"
            ),
        }
        tokio::select! {
            _ = tokio::time::sleep(chore.every()) => {}
            _ = stopped.changed() => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    struct Counted {
        done: AtomicU64,
        failing: bool,
        every: Duration,
    }

    impl Counted {
        fn every(every: Duration) -> Arc<Self> {
            Arc::new(Self {
                done: AtomicU64::new(0),
                failing: false,
                every,
            })
        }

        fn failing_every(every: Duration) -> Arc<Self> {
            Arc::new(Self {
                done: AtomicU64::new(0),
                failing: true,
                every,
            })
        }

        fn times(&self) -> u64 {
            self.done.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Chore for Counted {
        fn named(&self) -> &'static str {
            "counted"
        }

        fn every(&self) -> Duration {
            self.every
        }

        async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
            self.done.fetch_add(1, Ordering::SeqCst);
            if self.failing {
                return Err("the store went away".into());
            }

            Ok(1)
        }
    }

    const BRISKLY: Duration = Duration::from_millis(10);
    const HOURLY: Duration = Duration::from_secs(3_600);

    async fn until(reached: impl Fn() -> bool) {
        for _ in 0..500 {
            if reached() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    #[tokio::test]
    async fn a_chore_is_done_at_once_and_again_each_time_round() {
        let chore = Counted::every(BRISKLY);

        let chores = Chores::started(vec![chore.clone()]);
        until(|| chore.times() >= 3).await;
        chores.stop().await;

        assert!(chore.times() >= 3, "done {} times", chore.times());
    }

    #[tokio::test]
    async fn each_chore_comes_round_as_often_as_it_says() {
        let brisk = Counted::every(BRISKLY);
        let slow = Counted::every(HOURLY);

        let chores = Chores::started(vec![slow.clone(), brisk.clone()]);
        until(|| brisk.times() >= 3).await;
        chores.stop().await;

        assert!(brisk.times() >= 3);
        assert_eq!(slow.times(), 1, "an hour has not passed");
    }

    #[tokio::test]
    async fn a_chore_failing_keeps_neither_itself_nor_the_others_from_being_done_again() {
        let failing = Counted::failing_every(BRISKLY);
        let other = Counted::every(BRISKLY);

        let chores = Chores::started(vec![failing.clone(), other.clone()]);
        until(|| failing.times() >= 2 && other.times() >= 2).await;
        chores.stop().await;

        assert!(failing.times() >= 2);
        assert!(other.times() >= 2);
    }

    #[tokio::test]
    async fn stopped_chores_are_done_no_more() {
        let chore = Counted::every(BRISKLY);
        let chores = Chores::started(vec![chore.clone()]);
        until(|| chore.times() >= 1).await;

        chores.stop().await;
        let then = chore.times();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(chore.times(), then);
    }

    #[tokio::test]
    async fn stopping_does_not_wait_for_a_slow_chore_to_come_round() {
        let slow = Counted::every(HOURLY);
        let chores = Chores::started(vec![slow.clone()]);
        until(|| slow.times() >= 1).await;

        tokio::time::timeout(Duration::from_secs(5), chores.stop())
            .await
            .expect("stopping ends the wait at once");
    }
}
