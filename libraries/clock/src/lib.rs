use std::sync::Mutex;

use time::OffsetDateTime;

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

#[derive(Debug)]
pub struct FixedClock {
    now: Mutex<OffsetDateTime>,
}

impl FixedClock {
    pub fn new(now: OffsetDateTime) -> Self {
        Self {
            now: Mutex::new(now),
        }
    }

    pub fn set(&self, now: OffsetDateTime) {
        *self.now.lock().expect("clock lock poisoned") = now;
    }
}

impl Clock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        *self.now.lock().expect("clock lock poisoned")
    }
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    #[test]
    fn the_system_clock_reports_a_plausible_present() {
        assert!(SystemClock.now().unix_timestamp() > 1_700_000_000);
    }

    #[test]
    fn a_fixed_clock_reports_the_moment_it_was_given() {
        let clock = FixedClock::new(at(1_000));

        assert_eq!(clock.now(), at(1_000));
    }

    #[test]
    fn a_fixed_clock_can_be_moved_forward() {
        let clock = FixedClock::new(at(1_000));

        clock.set(at(2_000));

        assert_eq!(clock.now(), at(2_000));
    }
}
