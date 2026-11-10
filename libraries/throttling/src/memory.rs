use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use time::OffsetDateTime;

use crate::{Admission, Rule, Throttles, ThrottlesError, Tries};

type Counted = HashMap<(String, String), Tries>;

#[derive(Debug, Default)]
pub struct InMemoryThrottles {
    counted: Mutex<Counted>,
}

impl InMemoryThrottles {
    fn counted(&self) -> MutexGuard<'_, Counted> {
        self.counted.lock().expect("the throttles are not poisoned")
    }
}

#[async_trait]
impl Throttles for InMemoryThrottles {
    async fn tries(&self, guarded: &str, about: &str) -> Result<Option<Tries>, ThrottlesError> {
        Ok(self
            .counted()
            .get(&(guarded.to_owned(), about.to_owned()))
            .copied())
    }

    async fn admit(
        &self,
        guarded: &str,
        about: &str,
        rule: Rule,
        at: OffsetDateTime,
    ) -> Result<Admission, ThrottlesError> {
        let mut counted = self.counted();
        let key = (guarded.to_owned(), about.to_owned());

        match Tries::admitted(counted.get(&key).copied(), rule, at) {
            Ok(tries) => {
                counted.insert(key, tries);
                Ok(Admission::Admitted)
            }
            Err(wait) => Ok(Admission::Wait(wait)),
        }
    }

    async fn clear(&self, guarded: &str, about: &str) -> Result<(), ThrottlesError> {
        self.counted()
            .remove(&(guarded.to_owned(), about.to_owned()));

        Ok(())
    }

    async fn forget_tried_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, ThrottlesError> {
        let mut counted = self.counted();
        let mut old: Vec<(OffsetDateTime, (String, String))> = counted
            .iter()
            .filter(|(_, tries)| tries.last < before)
            .map(|(key, tries)| (tries.last, key.clone()))
            .collect();
        old.sort();
        old.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, key) in &old {
            counted.remove(key);
        }

        Ok(u64::try_from(old.len()).unwrap_or(u64::MAX))
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::InMemoryThrottles;
    use crate::suite::Workbench;

    struct InMemory(InMemoryThrottles);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryThrottles;

        async fn setup() -> Self {
            Self(InMemoryThrottles::default())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
