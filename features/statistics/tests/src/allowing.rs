use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::AllowingEverything;
use clock::FixedClock;
use statistics_core::{StatisticsCore, WordCountCatalog};
use transacting::Transactions;

/// The core as without accounts: everyone may do everything, whatever its own access has heard.
pub fn everyone(
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn WordCountCatalog>,
) -> StatisticsCore {
    StatisticsCore::new(
        transactions,
        catalog,
        KeptAccess::new(Arc::new(InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    )
}
