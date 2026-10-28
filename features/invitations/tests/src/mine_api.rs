use acting::Acting;
use axum::Extension;
use axum_test::TestServer;
use email::EmailAddress;
use eventsourcing::{AccountId, Agent, Version};
use invitations_contract::InvitationToYouDTO;
use invitations_core::{
    Invitation, InvitationCatalog, InvitationId, Person, ProjectLink, ProjectName, Role,
};
use time::Duration;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

fn as_account(account: &str, operator: bool) -> Acting {
    Acting {
        agent: Agent::Account(AccountId::from(account)),
        operator,
    }
}

fn grace() -> Acting {
    as_account("account_grace", true)
}

async fn known(wired: &Wired, account: &str, name: &str, email: &str) {
    wired
        .invitations
        .remember_person(&Person {
            account: AccountId::from(account),
            name: name.to_owned(),
            email: EmailAddress::new(email).expect("a plain address"),
        })
        .await
        .expect("remembering should succeed");
}

async fn listed(wired: &Wired, id: InvitationId) -> InvitationId {
    wired
        .invitations
        .catalogue(&id)
        .await
        .expect("cataloguing should succeed");

    id
}

async fn into(wired: &Wired, project: &str, role: Role, to: &str) -> InvitationId {
    let id = wired
        .invitations
        .invite_to_project(&grace(), project, role, to, ORIGIN)
        .await
        .expect("an owner may invite")
        .id;

    listed(wired, id).await
}

fn a_moment_later(wired: &Wired, seconds: i64) {
    wired.clock.set(wiring::at(1_700_000_000 + seconds));
}

async fn mine(wired: &Wired, acting: Acting) -> Vec<InvitationToYouDTO> {
    let asked = TestServer::new(
        invitations_rest::router(wired.invitations.clone()).layer(Extension(acting)),
    )
    .get("/invitations/mine")
    .await;
    asked.assert_status_ok();

    asked.json()
}

#[tokio::test]
async fn an_account_is_told_the_open_invitations_into_projects_to_its_address_newest_first() {
    let wired = wiring::wired();
    known(&wired, "account_ada", "Ada Lovelace", ADA).await;
    known(&wired, "account_grace", "Grace Hopper", "grace@example.com").await;
    wired
        .invitations
        .remember_project_name(&ProjectName {
            project: ProjectLink::from("project_loom"),
            version: Version::of(1),
            name: "The Loom".to_owned(),
        })
        .await
        .expect("remembering should succeed");

    let ran_out = into(&wired, "project_old", Role::Author, ADA).await;
    a_moment_later(&wired, 120);
    let into_the_loom = into(&wired, "project_loom", Role::Author, ADA).await;
    a_moment_later(&wired, 240);
    let into_the_weft = into(&wired, "project_weft", Role::Reader, "  Ada@Example.com ").await;
    a_moment_later(&wired, 360);
    into(&wired, "project_loom", Role::Reader, "hedy@example.com").await;
    let withdrawn = into(&wired, "project_shed", Role::Reader, ADA).await;
    wired
        .invitations
        .withdraw(&grace(), &withdrawn)
        .await
        .expect("an owner may withdraw");
    listed(&wired, withdrawn).await;
    a_moment_later(&wired, 480);
    let to_the_instance = wired
        .invitations
        .invite_to_instance(&grace(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id;
    listed(&wired, to_the_instance).await;
    let ran_out_until = wired
        .catalog
        .open_to(&EmailAddress::new(ADA).expect("a plain address"))
        .await
        .expect("asking should succeed")
        .into_iter()
        .find(|summary| summary.id == ran_out)
        .expect("listed")
        .until;
    wired.clock.set(ran_out_until + Duration::SECOND);

    let told = mine(&wired, as_account("account_ada", false)).await;

    let until_of = |seconds: i64| {
        clock::text::serialize(wiring::at(1_700_000_000 + seconds) + Invitation::LASTS)
    };
    assert_eq!(
        told,
        vec![
            InvitationToYouDTO {
                id: into_the_weft.to_string(),
                project: "project_weft".to_owned(),
                project_name: None,
                role: "reader".to_owned(),
                invited_by: Some("Grace Hopper".to_owned()),
                until: until_of(240),
            },
            InvitationToYouDTO {
                id: into_the_loom.to_string(),
                project: "project_loom".to_owned(),
                project_name: Some("The Loom".to_owned()),
                role: "author".to_owned(),
                invited_by: Some("Grace Hopper".to_owned()),
                until: until_of(120),
            },
        ],
        "neither the one that ran out, nor another's, nor the one withdrawn, nor the instance's"
    );
}

#[tokio::test]
async fn whoever_invitations_know_nothing_of_or_who_is_no_account_is_told_nothing() {
    let wired = wiring::wired();
    into(&wired, "project_loom", Role::Author, ADA).await;

    assert!(
        mine(&wired, as_account("account_ada", false))
            .await
            .is_empty()
    );
    assert!(
        mine(
            &wired,
            Acting {
                agent: Agent::System,
                operator: false,
            }
        )
        .await
        .is_empty()
    );
}
