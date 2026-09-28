use crate::Registry;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Registry;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

pub async fn an_unclaimed_key_is_taken_by_whoever_asks(registry: &impl Registry) {
    let held = registry
        .claim("board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");

    assert_eq!(held, "board_1");
}

pub async fn a_claimed_key_answers_with_who_holds_it(registry: &impl Registry) {
    registry
        .claim("board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");

    let held = registry
        .claim("board", "project_1", "board_2")
        .await
        .expect("a lost claim is not a failure");

    assert_eq!(
        held, "board_1",
        "the loser is told who won rather than being refused, so it can go and read that one"
    );
}

pub async fn claiming_the_same_key_twice_over_is_stable(registry: &impl Registry) {
    let mut held = Vec::new();
    for attempt in 1..=4 {
        held.push(
            registry
                .claim("board", "project_1", &format!("board_{attempt}"))
                .await
                .expect("claiming should succeed"),
        );
    }

    assert_eq!(held, vec!["board_1"; 4], "a claim never changes hands");
}

pub async fn each_key_is_claimed_on_its_own(registry: &impl Registry) {
    let mine = registry
        .claim("board", "project_mine", "board_1")
        .await
        .expect("claiming should succeed");
    let theirs = registry
        .claim("board", "project_theirs", "board_2")
        .await
        .expect("claiming should succeed");

    assert_eq!((mine.as_str(), theirs.as_str()), ("board_1", "board_2"));
}

pub async fn two_kinds_do_not_share_a_key(registry: &impl Registry) {
    let board = registry
        .claim("board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");
    let outline = registry
        .claim("outline", "project_1", "outline_1")
        .await
        .expect("claiming should succeed");

    assert_eq!(
        (board.as_str(), outline.as_str()),
        ("board_1", "outline_1"),
        "a project has one board and one outline, and neither claim may shadow the other"
    );
}

#[macro_export]
macro_rules! registry_conformance_case {
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
        $crate::registry_conformance_case!($workbench, an_unclaimed_key_is_taken_by_whoever_asks);
        $crate::registry_conformance_case!($workbench, a_claimed_key_answers_with_who_holds_it);
        $crate::registry_conformance_case!($workbench, claiming_the_same_key_twice_over_is_stable);
        $crate::registry_conformance_case!($workbench, each_key_is_claimed_on_its_own);
        $crate::registry_conformance_case!($workbench, two_kinds_do_not_share_a_key);
    };
}
