use futures_util::future;
use time::{Duration, OffsetDateTime};

use crate::{Admission, Rule, Throttles, Tries};

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
const FIVE_FREE: Rule = Rule { free: 5 };

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

async fn admitted<S: Throttles>(
    throttles: &S,
    guarded: &str,
    about: &str,
    second: i64,
) -> Admission {
    throttles
        .admit(guarded, about, FIVE_FREE, at(second))
        .await
        .expect("admitting should succeed")
}

pub async fn each_admitted_try_is_counted_and_keeps_its_moment<S: Throttles>(throttles: &S) {
    for second in [1_000, 1_060, 1_200] {
        assert_eq!(
            admitted(throttles, SIGNING_IN, ADA, second).await,
            Admission::Admitted
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

pub async fn a_try_refused_for_its_wait_is_told_it_and_not_counted<S: Throttles>(throttles: &S) {
    for _ in 0..5 {
        admitted(throttles, SIGNING_IN, ADA, 1_000).await;
    }

    let refused = admitted(throttles, SIGNING_IN, ADA, 1_010).await;
    let counted_then = throttles
        .tries(SIGNING_IN, ADA)
        .await
        .expect("reading should succeed");
    let once_waited = admitted(throttles, SIGNING_IN, ADA, 1_060).await;

    assert_eq!(refused, Admission::Wait(Duration::seconds(50)));
    assert_eq!(
        counted_then,
        Some(Tries {
            counted: 5,
            last: at(1_000),
        }),
        "a try refused for waiting must not count, or impatience alone would lengthen the wait"
    );
    assert_eq!(once_waited, Admission::Admitted);
}

pub async fn tries_a_day_old_are_forgotten_when_the_next_is_admitted<S: Throttles>(throttles: &S) {
    for about in [ADA, BEA] {
        for _ in 0..3 {
            admitted(throttles, SIGNING_IN, about, 1_000).await;
        }
    }
    let a_day_on = 1_000 + Tries::FORGOTTEN_AFTER.whole_seconds();

    admitted(throttles, SIGNING_IN, ADA, a_day_on - 1).await;
    admitted(throttles, SIGNING_IN, BEA, a_day_on).await;

    let counted = |about| async move {
        throttles
            .tries(SIGNING_IN, about)
            .await
            .expect("reading should succeed")
            .map(|tries| tries.counted)
    };
    assert_eq!(counted(ADA).await, Some(4));
    assert_eq!(
        counted(BEA).await,
        Some(1),
        "a try a day after the last starts the count over"
    );
}

pub async fn clearing_the_tries_starts_them_over<S: Throttles>(throttles: &S) {
    for _ in 0..3 {
        admitted(throttles, SIGNING_IN, ADA, 1_000).await;
    }

    throttles
        .clear(SIGNING_IN, ADA)
        .await
        .expect("clearing should succeed");
    admitted(throttles, SIGNING_IN, ADA, 2_000).await;

    assert_eq!(
        throttles
            .tries(SIGNING_IN, ADA)
            .await
            .expect("reading should succeed")
            .map(|tries| tries.counted),
        Some(1)
    );
}

pub async fn clearing_one_count_leaves_the_others<S: Throttles>(throttles: &S) {
    for (guarded, about) in [(SIGNING_IN, ADA), (SIGNING_IN, BEA), (SENDING_CODES, ADA)] {
        admitted(throttles, guarded, about, 1_000).await;
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
    for _ in 0..5 {
        admitted(throttles, SIGNING_IN, ADA, 1_000).await;
    }

    for (guarded, about) in [(SIGNING_IN, BEA), (SENDING_CODES, ADA)] {
        assert_eq!(
            admitted(throttles, guarded, about, 1_000).await,
            Admission::Admitted,
            "one person's mistakes must not slow another down, nor one kind of try another"
        );
    }
}

pub async fn tries_sent_at_once_are_admitted_no_more_than_the_rule_lets<S: Throttles>(
    throttles: &S,
) {
    let told =
        future::join_all((0..20).map(|_| throttles.admit(SIGNING_IN, ADA, FIVE_FREE, at(1_000))))
            .await;

    let admitted = told
        .into_iter()
        .map(|told| told.expect("admitting should succeed"))
        .filter(|told| *told == Admission::Admitted)
        .count();
    assert_eq!(
        admitted, 5,
        "tries sent together must not all read a count under the limit before any is counted"
    );
    assert_eq!(
        throttles
            .tries(SIGNING_IN, ADA)
            .await
            .expect("reading should succeed")
            .map(|tries| tries.counted),
        Some(5)
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
            each_admitted_try_is_counted_and_keeps_its_moment
        );
        $crate::throttles_conformance_case!(
            $workbench,
            a_try_refused_for_its_wait_is_told_it_and_not_counted
        );
        $crate::throttles_conformance_case!(
            $workbench,
            tries_a_day_old_are_forgotten_when_the_next_is_admitted
        );
        $crate::throttles_conformance_case!($workbench, clearing_the_tries_starts_them_over);
        $crate::throttles_conformance_case!($workbench, clearing_one_count_leaves_the_others);
        $crate::throttles_conformance_case!(
            $workbench,
            tries_are_counted_apart_by_whom_and_by_what_they_guard
        );
        $crate::throttles_conformance_case!(
            $workbench,
            tries_sent_at_once_are_admitted_no_more_than_the_rule_lets
        );
    };
}
