use futures_util::future;
use time::Duration;

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
const CY: &str = "cy@example.com";
const FIVE_FREE: Rule = Rule::free(5);

pub async fn nothing_counted_is_no_tries<S: Throttles>(throttles: &S) {
    // expect: nothing counted reads as no tries
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
        .admit(guarded, about, FIVE_FREE, clock::at(second))
        .await
        .expect("admitting should succeed")
}

pub async fn each_admitted_try_is_counted_and_keeps_its_moment<S: Throttles>(throttles: &S) {
    // when: three tries are made
    for second in [1_000, 1_060, 1_200] {
        // then: each is admitted
        assert_eq!(
            admitted(throttles, SIGNING_IN, ADA, second).await,
            Admission::Admitted
        );
    }

    // and: all three are counted as of the last
    assert_eq!(
        throttles
            .tries(SIGNING_IN, ADA)
            .await
            .expect("reading should succeed"),
        Some(Tries {
            counted: 3,
            last: clock::at(1_200),
        })
    );
}

pub async fn a_try_refused_for_its_wait_is_told_it_and_not_counted<S: Throttles>(throttles: &S) {
    // given: every free try used
    for _ in 0..5 {
        admitted(throttles, SIGNING_IN, ADA, 1_000).await;
    }

    // when: a try is made before the wait is over
    let refused = admitted(throttles, SIGNING_IN, ADA, 1_010).await;
    let counted_then = throttles
        .tries(SIGNING_IN, ADA)
        .await
        .expect("reading should succeed");
    // and: another once it is over
    let once_waited = admitted(throttles, SIGNING_IN, ADA, 1_060).await;

    // then: the early one is told its wait
    assert_eq!(refused, Admission::Wait(Duration::seconds(50)));
    // and: is not counted
    assert_eq!(
        counted_then,
        Some(Tries {
            counted: 5,
            last: clock::at(1_000),
        }),
        "a try refused for waiting must not count, or impatience alone would lengthen the wait"
    );
    // and: the one after the wait is admitted
    assert_eq!(once_waited, Admission::Admitted);
}

pub async fn tries_a_day_old_are_forgotten_when_the_next_is_admitted<S: Throttles>(throttles: &S) {
    // given: three tries each for two people
    for about in [ADA, BEA] {
        for _ in 0..3 {
            admitted(throttles, SIGNING_IN, about, 1_000).await;
        }
    }
    let a_day_on = 1_000 + Tries::FORGOTTEN_AFTER.whole_seconds();

    // when: one tries again just short of a day on and the other a day on
    admitted(throttles, SIGNING_IN, ADA, a_day_on - 1).await;
    admitted(throttles, SIGNING_IN, BEA, a_day_on).await;

    // then: the first counts on and the second starts over
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

pub async fn counts_last_tried_before_then_are_forgotten_oldest_first_and_no_more_than_asked<
    S: Throttles,
>(
    throttles: &S,
) {
    // given: three counts last tried at different moments
    admitted(throttles, SIGNING_IN, ADA, 1_000).await;
    admitted(throttles, SIGNING_IN, BEA, 2_000).await;
    admitted(throttles, SIGNING_IN, CY, 3_000).await;
    let still_there = |about| async move {
        throttles
            .tries(SIGNING_IN, about)
            .await
            .expect("reading should succeed")
            .is_some()
    };

    // when: one of those tried before the last is forgotten
    let forgotten = throttles
        .forget_tried_before(clock::at(3_000), 1)
        .await
        .expect("forgetting should succeed");

    // then: the oldest alone is gone
    assert_eq!(forgotten, 1);
    assert_eq!(
        (
            still_there(ADA).await,
            still_there(BEA).await,
            still_there(CY).await
        ),
        (false, true, true)
    );
    // when: all tried before the last are forgotten
    assert_eq!(
        throttles
            .forget_tried_before(clock::at(3_000), 10)
            .await
            .expect("forgetting should succeed"),
        1
    );
    // then: the last tried is kept
    assert!(
        still_there(CY).await,
        "a count tried at the moment itself is kept"
    );
}

pub async fn clearing_the_tries_starts_them_over<S: Throttles>(throttles: &S) {
    // given: three tries counted
    for _ in 0..3 {
        admitted(throttles, SIGNING_IN, ADA, 1_000).await;
    }

    // when: they are cleared
    throttles
        .clear(SIGNING_IN, ADA)
        .await
        .expect("clearing should succeed");
    // and: another try is made
    admitted(throttles, SIGNING_IN, ADA, 2_000).await;

    // then: only that one is counted
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
    // given: tries counted for two people and two guards
    for (guarded, about) in [(SIGNING_IN, ADA), (SIGNING_IN, BEA), (SENDING_CODES, ADA)] {
        admitted(throttles, guarded, about, 1_000).await;
    }

    // when: one count is cleared
    throttles
        .clear(SIGNING_IN, ADA)
        .await
        .expect("clearing should succeed");

    // then: the others are still counted
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
    // given: one person's free tries at one guard used
    for _ in 0..5 {
        admitted(throttles, SIGNING_IN, ADA, 1_000).await;
    }

    // when: another person tries, and the same person at another guard
    for (guarded, about) in [(SIGNING_IN, BEA), (SENDING_CODES, ADA)] {
        // then: each is admitted
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
    // when: twenty tries are sent at once
    let told = future::join_all(
        (0..20).map(|_| throttles.admit(SIGNING_IN, ADA, FIVE_FREE, clock::at(1_000))),
    )
    .await;

    // then: only the free ones are admitted
    let admitted = told
        .into_iter()
        .map(|told| told.expect("admitting should succeed"))
        .filter(|told| *told == Admission::Admitted)
        .count();
    assert_eq!(
        admitted, 5,
        "tries sent together must not all read a count under the limit before any is counted"
    );
    // and: only they are counted
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
        $crate::throttles_conformance_case!(
            $workbench,
            counts_last_tried_before_then_are_forgotten_oldest_first_and_no_more_than_asked
        );
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
