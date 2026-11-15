use std::sync::Arc;

use time::OffsetDateTime;

use crate::{Admission, Rule, Throttles, ThrottlesError};

const THE_INSTANCE: &str = "instance";

/// A number of tries the whole instance allows each hour, whoever makes them.
/// Each hour is counted afresh.
/// Several features share one cap by holding clones of it, so their tries are counted together.
/// The count is kept in the throttles the cap was made with.
#[derive(Clone)]
pub struct HourlyCap {
    guarded: &'static str,
    each_hour: u32,
    throttles: Arc<dyn Throttles>,
}

impl HourlyCap {
    pub fn new(guarded: &'static str, each_hour: u32, throttles: Arc<dyn Throttles>) -> Self {
        Self {
            guarded,
            each_hour,
            throttles,
        }
    }

    /// The same cap, but allowing the given number of tries each hour instead.
    pub fn allowing(self, each_hour: u32) -> Self {
        Self { each_hour, ..self }
    }

    /// Counts one try in the current hour.
    /// Once the hour's tries are used up, it answers how long until the next hour begins.
    pub async fn admit(&self, at: OffsetDateTime) -> Result<Admission, ThrottlesError> {
        let (this_hour, left) = crate::hour_of(THE_INSTANCE, at);

        Ok(
            match self
                .throttles
                .admit(self.guarded, &this_hour, Rule::capped(self.each_hour), at)
                .await?
            {
                Admission::Admitted => Admission::Admitted,
                Admission::Wait(_) => Admission::Wait(left),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;
    use crate::InMemoryThrottles;

    #[tokio::test]
    async fn clones_of_a_cap_count_together_until_the_next_hour() {
        // given: a cap of two each hour, held twice, at twenty to the hour
        let at = clock::at(1_700_000_000);
        let one = HourlyCap::new("mailing", 2, Arc::new(InMemoryThrottles::default()));
        let other = one.clone();

        // when: each holder makes one try
        let first = one.admit(at).await.expect("admitting should succeed");
        let second = other.admit(at).await.expect("admitting should succeed");

        // then: both are admitted
        assert_eq!((first, second), (Admission::Admitted, Admission::Admitted));
        // and: a third waits until the next hour
        assert_eq!(
            one.admit(at).await.expect("admitting should succeed"),
            Admission::Wait(Duration::minutes(46) + Duration::seconds(40))
        );
        // and: in the next hour it is admitted
        assert_eq!(
            other
                .admit(at + Duration::minutes(47))
                .await
                .expect("admitting should succeed"),
            Admission::Admitted
        );
    }
}
