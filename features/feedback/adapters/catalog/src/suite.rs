use feedback_core::CatalogError;
use transacting::{Transaction, Transactions};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: Sync;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Catalog> {
        Committing {
            catalog: self.catalog(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

/// A catalog whose writes are each committed in a transaction of their own, as a listener's are.
pub struct Committing<'a, C> {
    pub(crate) catalog: &'a C,
    pub(crate) transactions: &'a dyn Transactions,
}

pub(crate) fn unreachable(failure: transacting::Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

impl<C> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }
}
