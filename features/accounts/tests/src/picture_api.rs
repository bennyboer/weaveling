use accounts_contract::{PictureRequest, SignedInDTO, refused};
use accounts_core::{AccountEvent, KIND, PictureId};
use axum::http::StatusCode;
use axum::http::header::COOKIE;
use axum_test::TestResponse;
use eventsourcing::{AggregateId, EventStore, Version};

use crate::totp_bench::{Served, refused_as, served};

impl Served {
    async fn show(&self, cookie: &str, picture: &str, drop_previous: bool) -> TestResponse {
        self.server
            .put("/accounts/current/picture")
            .add_header(COOKIE, cookie.to_owned())
            .json(&PictureRequest {
                picture: picture.to_owned(),
                drop_previous,
            })
            .await
    }

    async fn stop_showing(&self, cookie: &str, address: &str) -> TestResponse {
        self.server
            .delete(address)
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    async fn picture_shown(&self, cookie: &str) -> Option<String> {
        let response = self.current(cookie).await;
        response.assert_status_ok();

        response.json::<SignedInDTO>().picture
    }

    /// The last event told of Ada's account.
    async fn told_last(&self) -> AccountEvent {
        self.wired
            .store
            .read_from(&AggregateId::from(&self.ada), KIND, Version::of(1))
            .await
            .expect("reading should succeed")
            .last()
            .expect("Ada's account has events")
            .event
            .clone()
    }
}

fn picture(id: &str) -> PictureId {
    PictureId::new(id).expect("a plain id")
}

#[tokio::test]
async fn a_picture_shown_is_told_with_the_session_and_replaces_the_one_before() {
    // given: Ada signed in, showing no picture
    let served = served(|wired| wired.accounts.clone()).await;
    let cookie = served.signed_in().await;
    assert_eq!(served.picture_shown(&cookie).await, None);

    // when: she shows a picture
    served
        .show(&cookie, "asset_1", false)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: her session tells it
    assert_eq!(
        served.picture_shown(&cookie).await,
        Some("asset_1".to_owned())
    );
    // when: she replaces it, asking for the first to be deleted
    served
        .show(&cookie, "asset_2", true)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    // then: the new one is told
    assert_eq!(
        served.picture_shown(&cookie).await,
        Some("asset_2".to_owned())
    );
    // and: the change names the one before and asks for it to be deleted
    assert_eq!(
        served.told_last().await,
        AccountEvent::PictureChanged {
            picture: picture("asset_2"),
            previous: Some(picture("asset_1")),
            drop_previous: true,
        }
    );
}

#[tokio::test]
async fn a_picture_is_removed_kept_or_deleted_as_asked() {
    // given: Ada signed in, showing a picture
    let served = served(|wired| wired.accounts.clone()).await;
    let cookie = served.signed_in().await;
    served
        .show(&cookie, "asset_1", false)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // when: she stops showing it, asking for it to be deleted
    served
        .stop_showing(&cookie, "/accounts/current/picture?drop=true")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: no picture is told
    assert_eq!(served.picture_shown(&cookie).await, None);
    // and: the removal asks for it to be deleted
    assert_eq!(
        served.told_last().await,
        AccountEvent::PictureRemoved {
            picture: picture("asset_1"),
            drop: true,
        }
    );
    // and: removing it again, without asking for anything, changes nothing
    served
        .stop_showing(&cookie, "/accounts/current/picture")
        .await
        .assert_status(StatusCode::NO_CONTENT);
    assert_eq!(
        served.told_last().await,
        AccountEvent::PictureRemoved {
            picture: picture("asset_1"),
            drop: true,
        }
    );
}

#[tokio::test]
async fn what_does_not_look_like_an_id_is_refused_and_nobody_signed_in_shows_nothing() {
    // given: Ada signed in
    let served = served(|wired| wired.accounts.clone()).await;
    let cookie = served.signed_in().await;

    // expect: an id with signs of an address in it is refused
    refused_as(
        &served.show(&cookie, "../asset_1", false).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::PICTURE_ID_INVALID,
    );
    assert_eq!(served.picture_shown(&cookie).await, None);
    // and: nobody signed in is refused
    served
        .server
        .put("/accounts/current/picture")
        .json(&PictureRequest {
            picture: "asset_1".to_owned(),
            drop_previous: false,
        })
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}
