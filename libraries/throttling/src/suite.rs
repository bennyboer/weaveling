use futures_util::future;
use time::OffsetDateTime;

use crate::{Throttles, Tries};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Throttles;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

const SIGNING_IN: &str = "signing-in";
const SENDING_CODES: &str = "sending-codes";
const ADA: &str = "ada@example.com";
const BEA: &str = "bea@example.com";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub async fn nothing_counted_is_no_tries<S: Throttles>(throttles: &S) {
    assert_eq!(
        throttles
            .tries(SIGNING_IN, ADA)
            .await
            .expect("reading should succeed"),
        None
    );
}

pub async fn each_try_counted_raises_the_count_and_keeps_its_moment<S: Throttles>(throttles: &S) {
    for (second, counted) in [(1_000, 1), (1_060, 2), (1_200, 3)] {
        assert_eq!(
            throttles
                .count(SIGNING_IN, ADA, at(second))
                .await
                .expect("counting should succeed"),
            Tries {
                counted,
                last: at(second),
            }
        );
    }

    assert_eq!(
        throttles
            .tries(SIGNING_IN, ADA)
            .await
            .expect("reading should succeed"),
        Some(Tries {
            counted: 3,
            last: at(1_200),
        })
    );
}

pub async fn clearing_the_tries_starts_them_over<S: Throttles>(throttles: &S) {
    for _ in 0..3 {
        throttles
            .count(SIGNING_IN, ADA, at(1_000))
            .await
            .expect("counting should succeed");
    }

    throttles
        .clear(SIGNING_IN, ADA)
        .await
        .expect("clearing should succeed");

    assert_eq!(
        throttles
            .count(SIGNING_IN, ADA, at(2_000))
            .await
            .expect("counting should succeed")
            .counted,
        1
    );
}

pub async fn clearing_one_count_leaves_the_others<S: Throttles>(throttles: &S) {
    for (guarded, about) in [(SIGNING_IN, ADA), (SIGNING_IN, BEA), (SENDING_CODES, ADA)] {
        throttles
            .count(guarded, about, at(1_000))
            .await
            .expect("counting should succeed");
    }

    throttles
        .clear(SIGNING_IN, ADA)
        .await
        .expect("clearing should succeed");

    for (guarded, about) in [(SIGNING_IN, BEA), (SENDING_CODES, ADA)] {
        assert!(
            throttles
                .tries(guarded, about)
                .await
                .expect("reading should succeed")
                .is_some(),
            "signing in rightly clears one's own count, not another's, nor what else is counted"
        );
    }
}

pub async fn tries_are_counted_apart_by_whom_and_by_what_they_guard<S: Throttles>(throttles: &S) {
    throttles
        .count(SIGNING_IN, ADA, at(1_000))
        .await
        .expect("counting should succeed");

    for (guarded, about) in [(SIGNING_IN, BEA), (SENDING_CODES, ADA)] {
        assert_eq!(
            throttles
                .tries(guarded, about)
                .await
                .expect("reading should succeed"),
            None,
            "one person's mistakes must not slow another down, nor one kind of try another"
        );
    }
}

pub async fn tries_counted_at_once_are_each_counted<S: Throttles>(throttles: &S) {
    let counted =
        future::join_all((0..20).map(|_| throttles.count(SIGNING_IN, ADA, at(1_000)))).await;

    assert!(counted.iter().all(Result::is_ok));
    assert_eq!(
        throttles
            .tries(SIGNING_IN, ADA)
            .await
            .expect("reading should succeed")
            .map(|tries| tries.counted),
        Some(20),
        "tries sent together must each be counted, or sending them together slips the throttle"
    );
}

#[macro_export]
macro_rules! throttles_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::throttles_conformance_case!($workbench, nothing_counted_is_no_tries);
        $crate::throttles_conformance_case!(
            $workbench,
            each_try_counted_raises_the_count_and_keeps_its_moment
        );
        $crate::throttles_conformance_case!($workbench, clearing_the_tries_starts_them_over);
        $crate::throttles_conformance_case!($workbench, clearing_one_count_leaves_the_others);
        $crate::throttles_conformance_case!(
            $workbench,
            tries_are_counted_apart_by_whom_and_by_what_they_guard
        );
        $crate::throttles_conformance_case!($workbench, tries_counted_at_once_are_each_counted);
    };
}
