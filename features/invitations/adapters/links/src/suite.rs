use invitations_core::{InvitationId, InvitationLinks, InvitationToken};
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: InvitationLinks;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

fn an_invitation(seconds: i64) -> InvitationId {
    InvitationId::generate(
        OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment"),
    )
}

pub async fn a_kept_link_names_its_invitation<S: InvitationLinks>(links: &S) {
    let invitation = an_invitation(1_000);
    let token = InvitationToken::drawn();

    links
        .keep(&token.digest(), &invitation)
        .await
        .expect("keeping should succeed");

    assert_eq!(
        links
            .invitation_of(&token.digest())
            .await
            .expect("reading should succeed"),
        Some(invitation)
    );
}

pub async fn a_link_never_kept_names_nothing<S: InvitationLinks>(links: &S) {
    links
        .keep(&InvitationToken::drawn().digest(), &an_invitation(1_000))
        .await
        .expect("keeping should succeed");

    assert_eq!(
        links
            .invitation_of(&InvitationToken::drawn().digest())
            .await
            .expect("reading should succeed"),
        None
    );
}

pub async fn a_forgotten_invitation_is_named_by_its_link_no_more<S: InvitationLinks>(links: &S) {
    let (ada, grace) = (an_invitation(1_000), an_invitation(2_000));
    let (to_ada, to_grace) = (InvitationToken::drawn(), InvitationToken::drawn());
    links
        .keep(&to_ada.digest(), &ada)
        .await
        .expect("keeping should succeed");
    links
        .keep(&to_grace.digest(), &grace)
        .await
        .expect("keeping should succeed");

    links.forget(&ada).await.expect("forgetting should succeed");

    assert_eq!(
        links
            .invitation_of(&to_ada.digest())
            .await
            .expect("reading should succeed"),
        None
    );
    assert_eq!(
        links
            .invitation_of(&to_grace.digest())
            .await
            .expect("reading should succeed"),
        Some(grace),
        "forgetting one invitation leaves the others' links"
    );
}

pub async fn forgetting_an_invitation_never_kept_is_harmless<S: InvitationLinks>(links: &S) {
    links
        .forget(&an_invitation(1_000))
        .await
        .expect("forgetting nothing should succeed");
}

#[macro_export]
macro_rules! links_conformance_case {
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
        $crate::links_conformance_case!($workbench, a_kept_link_names_its_invitation);
        $crate::links_conformance_case!($workbench, a_link_never_kept_names_nothing);
        $crate::links_conformance_case!(
            $workbench,
            a_forgotten_invitation_is_named_by_its_link_no_more
        );
        $crate::links_conformance_case!(
            $workbench,
            forgetting_an_invitation_never_kept_is_harmless
        );
    };
}
