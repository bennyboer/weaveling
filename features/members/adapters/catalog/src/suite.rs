use eventsourcing::{AccountId, Version};
use members_core::{Member, Membership, MembershipCatalog, ProjectLink, ProjectMembers, Role};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: MembershipCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

fn ada() -> AccountId {
    AccountId::from("account_ada")
}

fn grace() -> AccountId {
    AccountId::from("account_grace")
}

fn hedy() -> AccountId {
    AccountId::from("account_hedy")
}

fn a_project(named: &str) -> ProjectLink {
    ProjectLink::from(named)
}

pub fn members_of(project: &str, version: u64, members: &[(AccountId, Role)]) -> ProjectMembers {
    ProjectMembers {
        project: a_project(project),
        version: Version::of(version),
        members: members
            .iter()
            .map(|(account, role)| Member {
                account: account.clone(),
                role: *role,
            })
            .collect(),
    }
}

async fn remembered(catalog: &impl MembershipCatalog, members: &ProjectMembers) {
    catalog
        .remember(members)
        .await
        .expect("remembering should succeed");
}

async fn role(
    catalog: &impl MembershipCatalog,
    project: &str,
    account: &AccountId,
) -> Option<Role> {
    catalog
        .role_of(&a_project(project), account)
        .await
        .expect("asking for a role should succeed")
}

async fn memberships(catalog: &impl MembershipCatalog, account: &AccountId) -> Vec<Membership> {
    catalog
        .memberships_of(account)
        .await
        .expect("listing memberships should succeed")
}

pub async fn an_empty_catalog_knows_nobody_in_any_project(catalog: &impl MembershipCatalog) {
    assert_eq!(role(catalog, "project_1", &ada()).await, None);
    assert!(memberships(catalog, &ada()).await.is_empty());
}

pub async fn each_member_has_their_role_in_their_project_and_nowhere_else(
    catalog: &impl MembershipCatalog,
) {
    remembered(
        catalog,
        &members_of(
            "project_1",
            2,
            &[(ada(), Role::Owner), (grace(), Role::Reader)],
        ),
    )
    .await;
    remembered(
        catalog,
        &members_of("project_2", 1, &[(hedy(), Role::Owner)]),
    )
    .await;

    assert_eq!(role(catalog, "project_1", &ada()).await, Some(Role::Owner));
    assert_eq!(
        role(catalog, "project_1", &grace()).await,
        Some(Role::Reader)
    );
    assert_eq!(role(catalog, "project_1", &hedy()).await, None);
    assert_eq!(role(catalog, "project_2", &ada()).await, None);
    assert_eq!(role(catalog, "project_3", &ada()).await, None);
}

pub async fn an_account_s_memberships_are_every_project_it_is_in_by_project(
    catalog: &impl MembershipCatalog,
) {
    remembered(
        catalog,
        &members_of("project_b", 1, &[(ada(), Role::Owner)]),
    )
    .await;
    remembered(
        catalog,
        &members_of("project_c", 1, &[(grace(), Role::Owner)]),
    )
    .await;
    remembered(
        catalog,
        &members_of(
            "project_a",
            2,
            &[(grace(), Role::Owner), (ada(), Role::Author)],
        ),
    )
    .await;

    assert_eq!(
        memberships(catalog, &ada()).await,
        vec![
            Membership {
                project: a_project("project_a"),
                role: Role::Author,
            },
            Membership {
                project: a_project("project_b"),
                role: Role::Owner,
            },
        ]
    );
}

pub async fn newer_members_replace_the_older_whoever_is_gone_with_them(
    catalog: &impl MembershipCatalog,
) {
    remembered(
        catalog,
        &members_of(
            "project_1",
            2,
            &[(ada(), Role::Owner), (grace(), Role::Author)],
        ),
    )
    .await;

    remembered(
        catalog,
        &members_of(
            "project_1",
            4,
            &[(ada(), Role::Reader), (hedy(), Role::Owner)],
        ),
    )
    .await;

    assert_eq!(role(catalog, "project_1", &ada()).await, Some(Role::Reader));
    assert_eq!(role(catalog, "project_1", &grace()).await, None);
    assert!(memberships(catalog, &grace()).await.is_empty());
    assert_eq!(role(catalog, "project_1", &hedy()).await, Some(Role::Owner));
}

pub async fn older_members_arriving_late_change_nothing(catalog: &impl MembershipCatalog) {
    remembered(
        catalog,
        &members_of(
            "project_1",
            4,
            &[(ada(), Role::Owner), (grace(), Role::Author)],
        ),
    )
    .await;

    for late in [2, 4] {
        remembered(
            catalog,
            &members_of(
                "project_1",
                late,
                &[(ada(), Role::Owner), (hedy(), Role::Reader)],
            ),
        )
        .await;
    }

    assert_eq!(
        role(catalog, "project_1", &grace()).await,
        Some(Role::Author)
    );
    assert_eq!(role(catalog, "project_1", &hedy()).await, None);
}

#[macro_export]
macro_rules! catalog_conformance_case {
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
        $crate::catalog_conformance_case!($workbench, an_empty_catalog_knows_nobody_in_any_project);
        $crate::catalog_conformance_case!(
            $workbench,
            each_member_has_their_role_in_their_project_and_nowhere_else
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_account_s_memberships_are_every_project_it_is_in_by_project
        );
        $crate::catalog_conformance_case!(
            $workbench,
            newer_members_replace_the_older_whoever_is_gone_with_them
        );
        $crate::catalog_conformance_case!($workbench, older_members_arriving_late_change_nothing);
    };
}
