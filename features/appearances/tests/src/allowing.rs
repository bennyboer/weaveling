use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use appearances_core::{AppearanceCatalog, AppearancesCore};
use authorizing::AllowingEverything;
use clock::FixedClock;
use transacting::Transactions;

/// Builds the core as it runs without accounts, where everyone may do everything.
/// This holds no matter what its own access records contain.
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
