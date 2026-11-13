use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use appearances_core::{AppearanceCatalog, AppearancesCore};
use authorizing::AllowingEverything;
use clock::FixedClock;
use transacting::Transactions;

/// The core as without accounts: everyone may do everything, whatever its own access has heard.
pub fn everyone(
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn AppearanceCatalog>,
) -> AppearancesCore {
    AppearancesCore::new(
        transactions,
        catalog,
        KeptAccess::new(Arc::new(InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    )
}
