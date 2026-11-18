use std::sync::Arc;

use crate::sample::SampleBook;
use access::{InMemoryAccess, KeptAccess};
use authorizing::AllowingEverything;
use clock::FixedClock;
use readings_core::{NobodyFollows, ReadingsCore};
use transacting::InMemoryTransactions;

/// Builds the core as it runs without accounts, where everyone may do everything, around the sample book.
/// This holds no matter what its own access records contain.
pub fn everyone() -> ReadingsCore {
    ReadingsCore::new(
        Arc::new(SampleBook),
        Arc::new(SampleBook),
        Arc::new(NobodyFollows),
        Arc::new(InMemoryTransactions::new()),
        KeptAccess::new(Arc::new(InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    )
}
