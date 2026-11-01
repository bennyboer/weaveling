use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::Duration;

pub const TRIES: u8 = 5;
const FIRST_PAUSE: Duration = Duration::from_millis(10);

pub trait LostRace {
    fn lost_race(&self) -> bool;
}

#[macro_export]
macro_rules! retrying {
    ($transactions:expr, |$transaction:ident| $work:block) => {
        $crate::retrying!($transactions, $crate::retrying::TRIES, |$transaction| $work)
    };
    ($transactions:expr, $tries:expr, |$transaction:ident| $work:block) => {{
        let transactions: &dyn $crate::Transactions = $transactions;
        let tries: u8 = $tries;
        let mut tried = 1;
        loop {
            let outcome = async {
                let mut owned = transactions.begin().await?;
                let worked = {
                    let $transaction = &mut owned;
                    async $work.await
                };
                let done = match worked {
                    Ok(done) => done,
                    Err(failure) => return Err(failure),
                };
                owned.finish().await?;
                Ok(done)
            }
            .await;
            match outcome {
                Err(failure) if $crate::retrying::again(&failure, tried, tries) => {
                    $crate::retrying::pause(tried).await;
                    tried += 1;
                }
                outcome => break outcome,
            }
        }
    }};
}

#[doc(hidden)]
pub fn again(failure: &impl LostRace, tried: u8, tries: u8) -> bool {
    failure.lost_race() && tried < tries
}

#[doc(hidden)]
pub async fn pause(tried: u8) {
    tokio::time::sleep(crate::retrying::pause_after(tried)).await;
}

pub(crate) fn pause_after(tried: u8) -> Duration {
    let longest = FIRST_PAUSE * (1 << (tried - 1));
    let drawn = RandomState::new().build_hasher().finish();

    longest.mul_f64((drawn % 1_000) as f64 / 1_000.0)
}
