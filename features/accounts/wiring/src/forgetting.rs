use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use accounts_core::AccountsCore;
use async_trait::async_trait;
use chores::Chore;
use clock::Clock;

const EACH_BATCH: u64 = 500;

/// How long a session or a code is kept after it expires.
/// So if one is used a little late, the user is told that it expired, rather than that it does not exist.
const KEPT_AFTER_RUNNING_OUT: time::Duration = time::Duration::days(1);

/// Every hour, forgets sessions and codes that expired a day ago or more.
/// Nothing else ever deletes them.
pub struct ForgetRunOutSessionsAndCodes {
    accounts: AccountsCore,
    clock: Arc<dyn Clock>,
}

impl ForgetRunOutSessionsAndCodes {
    pub fn new(accounts: AccountsCore, clock: Arc<dyn Clock>) -> Self {
        Self { accounts, clock }
    }
}

#[async_trait]
impl Chore for ForgetRunOutSessionsAndCodes {
    fn named(&self) -> &'static str {
        "forget-run-out-sessions-and-codes"
    }

    fn every(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let before = self.clock.now() - KEPT_AFTER_RUNNING_OUT;
        let mut forgotten = 0;

        loop {
            let (sessions, codes) = self
                .accounts
                .forget_run_out_before(before, EACH_BATCH)
                .await?;
            forgotten += sessions + codes;
            if sessions < EACH_BATCH && codes < EACH_BATCH {
                return Ok(forgotten);
            }
        }
    }
}
