use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::Duration;

use crate::Failure;

pub const TRIES: u8 = 8;
const FIRST_PAUSE: Duration = Duration::from_millis(10);

pub trait LostRace {
    fn lost_race(&self) -> bool;
}

/// What a try under `retrying!` answered when it failed: the work's own refusal, which is never tried again, or its transaction's failure, which is when the transaction lost a race. Lets work whose error knows nothing of transactions, as an adapter's, be tried again all the same.
#[derive(Debug)]
pub enum Tried<E> {
    Refused(E),
    Failed(Failure),
}

impl<E> Tried<E> {
    pub fn answered(self, failed: impl FnOnce(Failure) -> E) -> E {
        match self {
            Tried::Refused(refused) => refused,
            Tried::Failed(failure) => failed(failure),
        }
    }
}

impl<E> LostRace for Tried<E> {
    fn lost_race(&self) -> bool {
        matches!(self, Tried::Failed(failure) if failure.lost_race())
    }
}

impl<E> From<Failure> for Tried<E> {
    fn from(failure: Failure) -> Self {
        Tried::Failed(failure)
    }
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
                let mut owned = match transactions.begin().await {
                    Ok(owned) => owned,
                    Err(failure) => return Err((failure.into(), false)),
                };
                let worked = {
                    let $transaction = &mut owned;
                    async $work.await
                };
                match worked {
                    Ok(done) => match owned.finish().await {
                        Ok(()) => Ok(done),
                        Err(failure) => {
                            let lost = $crate::LostRace::lost_race(&failure);
                            Err((failure.into(), lost))
                        }
                    },
                    Err(failure) => {
                        let lost = owned.lost_race();
                        Err((failure, lost))
                    }
                }
            }
            .await;
            match outcome {
                Ok(done) => break Ok(done),
                Err((failure, lost)) if $crate::retrying::again(&failure, lost, tried, tries) => {
                    $crate::retrying::pause(tried).await;
                    tried += 1;
                }
                Err((failure, _)) => break Err(failure),
            }
        }
    }};
}

/// Runs the work in the transaction it is handed, or, handed none, in one of its own through `retrying!`: a transaction borrowed is the owner's to try again.
#[macro_export]
macro_rules! joining {
    ($transactions:expr, $within:expr, |$transaction:ident| $work:block) => {
        match $within {
            Some($transaction) => async $work.await,
            None => $crate::retrying!($transactions, |$transaction| $work),
        }
    };
}

#[doc(hidden)]
pub fn again(failure: &impl LostRace, lost: bool, tried: u8, tries: u8) -> bool {
    (lost && tried < TRIES) || (failure.lost_race() && tried < tries)
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
