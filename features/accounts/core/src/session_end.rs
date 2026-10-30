use std::sync::Arc;

use acting::{Ending, SessionEnd};
use async_trait::async_trait;
use time::OffsetDateTime;

use crate::accounts::AccountsCore;
use crate::session::SessionToken;

struct EndingSession {
    accounts: AccountsCore,
    token: SessionToken,
    until: OffsetDateTime,
}

pub(crate) fn of(accounts: AccountsCore, token: SessionToken, until: OffsetDateTime) -> SessionEnd {
    SessionEnd::of(Arc::new(EndingSession {
        accounts,
        token,
        until,
    }))
}

#[async_trait]
impl Ending for EndingSession {
    async fn reached(&self) {
        let mut checks = self.accounts.session_checks();
        let left = self.until - self.accounts.now();
        let deadline = tokio::time::sleep(std::time::Duration::try_from(left).unwrap_or_default());
        tokio::pin!(deadline);

        loop {
            if !self.accounts.still_acting(&self.token).await {
                return;
            }
            tokio::select! {
                () = &mut deadline => return,
                _ = checks.changed() => {}
            }
        }
    }
}
