use accounts_contract::{
    RecoveryCodesDTO, SecondFactorCodeRequest, SettingUpSecondFactorDTO, SignInRequest, SignedInDTO,
};
use accounts_core::{AccountId, AccountsCore, EmailAddress, EmailAddresses, TotpKey};
use axum::http::StatusCode;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum_test::{TestResponse, TestServer};
use serde_json::Value;
use totp_rs::{Builder, Secret};

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

pub const KEY: &str = "7e57000000000000000000000000000000000000000000000000000000007e57";

pub struct Served {
    pub wired: Wired,
    pub server: TestServer,
    pub ada: AccountId,
}

pub async fn served(accounts: impl Fn(&Wired) -> AccountsCore) -> Served {
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    wired
        .accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the code sent should verify");
    let ada = wired
        .secrets
        .holder(&EmailAddress::new(ADA).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held");
    let core = accounts(&wired);
    let server = TestServer::new(accounts_rest::router(core.clone()).merge(
        accounts_rest::admitting_only_the_signed_in(
            core.clone(),
            accounts_rest::guarded_router(core),
        ),
    ));

    Served { wired, server, ada }
}

pub async fn with_a_key() -> Served {
    served(|wired| {
        wired
            .accounts
            .clone()
            .sealing_second_factors(TotpKey::from_hex(KEY).expect("a test key"))
    })
    .await
}

pub fn code_of(secret: &str, at: i64) -> String {
    Builder::new()
        .with_secret(Secret::try_from_base32(secret).expect("the secret is base32"))
        .build()
        .expect("a secret of twenty bytes")
        .generate(u64::try_from(at).expect("after 1970"))
        .to_string()
}

pub fn refused_as(response: &TestResponse, status: StatusCode, kind: &str) {
    response.assert_status(status);
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");
}

pub fn cookie_set_by(response: &TestResponse) -> String {
    response
        .header(SET_COOKIE)
        .to_str()
        .expect("a cookie is plain text")
        .split(';')
        .next()
        .expect("the cookie names the session")
        .to_owned()
}

impl Served {
    pub async fn signed_in(&self) -> String {
        cookie_set_by(
            &self
                .server
                .post("/sessions")
                .json(&SignInRequest {
                    email: ADA.to_owned(),
                    password: HER_PASSWORD.to_owned(),
                })
                .await,
        )
    }

    pub async fn begin(&self, cookie: &str) -> TestResponse {
        self.server
            .post("/accounts/current/second-factor/setup")
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    pub async fn begun(&self, cookie: &str) -> SettingUpSecondFactorDTO {
        let response = self.begin(cookie).await;
        response.assert_status_ok();

        response.json()
    }

    pub async fn finish(&self, cookie: &str, code: &str) -> TestResponse {
        self.server
            .post("/accounts/current/second-factor/setup/finish")
            .add_header(COOKIE, cookie.to_owned())
            .json(&SecondFactorCodeRequest {
                code: code.to_owned(),
            })
            .await
    }

    pub async fn current(&self, cookie: &str) -> TestResponse {
        self.server
            .get("/sessions/current")
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    pub async fn second_factor_shown(
        &self,
        cookie: &str,
    ) -> Option<accounts_contract::SecondFactorDTO> {
        let response = self.current(cookie).await;
        response.assert_status_ok();

        response.json::<SignedInDTO>().second_factor
    }

    pub fn at(&self, seconds: i64) -> i64 {
        self.wired.clock.set(clock::at(seconds));

        seconds
    }

    pub async fn answer(&self, owing: &str, code: &str) -> TestResponse {
        self.server
            .post("/sessions/current/second-factor")
            .add_header(COOKIE, owing.to_owned())
            .json(&SecondFactorCodeRequest {
                code: code.to_owned(),
            })
            .await
    }

    pub async fn answer_with_recovery_code(&self, owing: &str, code: &str) -> TestResponse {
        self.server
            .post("/sessions/current/recovery-code")
            .add_header(COOKIE, owing.to_owned())
            .json(&SecondFactorCodeRequest {
                code: code.to_owned(),
            })
            .await
    }

    pub async fn draw_recovery_codes(&self, cookie: &str, code: &str) -> TestResponse {
        self.server
            .post("/accounts/current/second-factor/recovery-codes")
            .add_header(COOKIE, cookie.to_owned())
            .json(&SecondFactorCodeRequest {
                code: code.to_owned(),
            })
            .await
    }

    pub async fn stage_of(&self, cookie: &str) -> String {
        let response = self.current(cookie).await;
        response.assert_status_ok();

        response.json::<SignedInDTO>().stage
    }
}

/// Ada with a second factor set up at the given moment: its secret, the recovery codes she was shown, and the cookie of the session that set it up.
pub struct SetUp {
    pub served: Served,
    pub secret: String,
    pub recovery_codes: Vec<String>,
    pub cookie: String,
}

pub async fn with_a_second_factor_set_up_at(at: i64) -> SetUp {
    let served = with_a_key().await;
    let cookie = served.signed_in().await;
    let setting_up = served.begun(&cookie).await;
    let now = served.at(at);
    let finished = served
        .finish(&cookie, &code_of(&setting_up.secret, now))
        .await;
    finished.assert_status_ok();
    let shown: RecoveryCodesDTO = finished.json();

    SetUp {
        cookie: cookie_set_by(&finished),
        served,
        secret: setting_up.secret,
        recovery_codes: shown.codes,
    }
}
