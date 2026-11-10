use std::cell::RefCell;
use std::rc::Rc;

use accounts_contract::refused::NOT_SIGNED_IN;
use gloo_net::http::{RequestBuilder, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

const PROBLEM: &str = "application/problem+json";
const IF_MATCH: &str = "If-Match";

thread_local! {
    static NO_LONGER_SIGNED_IN: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

pub(crate) fn when_no_longer_signed_in(told: impl Fn() + 'static) {
    NO_LONGER_SIGNED_IN.with(|kept| *kept.borrow_mut() = Some(Rc::new(told)));
}

fn no_longer_signed_in() {
    let told = NO_LONGER_SIGNED_IN.with(|kept| kept.borrow().clone());
    if let Some(told) = told {
        told();
    }
}

fn refused_as(failed: &ApiError, kind: &str) -> bool {
    matches!(failed, ApiError::Rejected(refusal) if refusal.kind.as_deref() == Some(kind))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    Rejected(Refusal),
    NotFound(&'static str),
    Outdated(&'static str),
    Overtaken,
    Offline,
    Unexpected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub kind: Option<String>,
    pub detail: String,
    pub particulars: Vec<(String, Particular)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Particular {
    Count(i64),
    Text(String),
    Texts(Vec<String>),
}

pub(crate) fn made_on(request: RequestBuilder, version: u64) -> RequestBuilder {
    request.header(IF_MATCH, &format!("\"{version}\""))
}

pub(crate) async fn sent(request: RequestBuilder) -> Result<Response, ApiError> {
    request.send().await.map_err(|_| ApiError::Offline)
}

pub(crate) async fn sent_with(
    request: RequestBuilder,
    body: &impl Serialize,
) -> Result<Response, ApiError> {
    request
        .json(body)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)
}

pub(crate) async fn parsed<T: DeserializeOwned>(
    response: Response,
    subject: &'static str,
) -> Result<T, ApiError> {
    checked(response, subject)
        .await?
        .json()
        .await
        .map_err(|_| ApiError::Unexpected)
}

pub(crate) async fn checked(
    response: Response,
    subject: &'static str,
) -> Result<Response, ApiError> {
    if response.ok() {
        return Ok(response);
    }

    match response.status() {
        401 => {
            let failed = rejection(response).await;
            if refused_as(&failed, NOT_SIGNED_IN) {
                no_longer_signed_in();
            }
            Err(failed)
        }
        400 | 403 | 409 | 422 | 429 => Err(rejection(response).await),
        404 => Err(ApiError::NotFound(subject)),
        412 => Err(ApiError::Outdated(subject)),
        _ => Err(ApiError::Unexpected),
    }
}

pub(crate) async fn rejection(response: Response) -> ApiError {
    let status = response.status();
    let a_problem = response
        .headers()
        .get("content-type")
        .is_some_and(|kind| kind.starts_with(PROBLEM));
    let Ok(body) = response.text().await else {
        return ApiError::Unexpected;
    };

    let refusal = match a_problem {
        true => serde_json::from_str(&body).ok().and_then(refusal_from),
        false if status == 400 && !body.trim().is_empty() => Some(Refusal {
            kind: None,
            detail: body,
            particulars: Vec::new(),
        }),
        false => None,
    };

    refusal.map_or(ApiError::Unexpected, ApiError::Rejected)
}

fn refusal_from(mut told: Map<String, Value>) -> Option<Refusal> {
    let detail = told.remove("detail")?.as_str()?.to_owned();
    let kind = told
        .remove("type")
        .and_then(|kind| kind.as_str().map(str::to_owned));
    told.remove("status");
    let particulars = told
        .into_iter()
        .filter_map(|(name, value)| {
            let particular = match value {
                Value::Number(count) => Particular::Count(count.as_i64()?),
                Value::String(text) => Particular::Text(text),
                Value::Array(listed) => Particular::Texts(
                    listed
                        .into_iter()
                        .map(|each| each.as_str().map(str::to_owned))
                        .collect::<Option<_>>()?,
                ),
                _ => return None,
            };

            Some((name, particular))
        })
        .collect();

    Some(Refusal {
        kind,
        detail,
        particulars,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn told(document: Value) -> Option<Refusal> {
        match document {
            Value::Object(told) => refusal_from(told),
            _ => None,
        }
    }

    #[test]
    fn a_problem_tells_its_kind_its_detail_and_its_particulars() {
        // when: a problem with a kind, a detail and particulars is told
        let refusal = told(json!({
            "type": "project-name-too-long",
            "status": 400,
            "detail": "project name must be at most 200 characters, got 240",
            "max": 200,
            "actual": 240,
            "name": "The loom",
        }));

        // then: the refusal carries all three, particulars in order of their names
        assert_eq!(
            refusal,
            Some(Refusal {
                kind: Some("project-name-too-long".to_owned()),
                detail: "project name must be at most 200 characters, got 240".to_owned(),
                particulars: vec![
                    ("actual".to_owned(), Particular::Count(240)),
                    ("max".to_owned(), Particular::Count(200)),
                    ("name".to_owned(), Particular::Text("The loom".to_owned())),
                ],
            })
        );
    }

    #[test]
    fn a_problem_without_a_kind_still_tells_its_detail() {
        // when: a problem with a detail and no kind is told
        let refusal = told(json!({ "detail": "refused" })).expect("a refusal");

        // then: it is a refusal of no kind with that detail
        assert_eq!(refusal.kind, None);
        assert_eq!(refusal.detail, "refused");
    }

    #[test]
    fn a_problem_without_a_detail_is_no_refusal() {
        // expect: a problem with a kind and no detail tells no refusal
        assert_eq!(told(json!({ "type": "project-deleted" })), None);
    }

    #[test]
    fn what_is_neither_a_count_nor_a_text_is_left_out() {
        // when: a problem with a fraction, a flag and a list is told
        let refusal = told(json!({
            "type": "kind",
            "detail": "refused",
            "fraction": 0.5,
            "flag": true,
            "list": [1],
        }))
        .expect("a refusal");

        // then: none of them is a particular
        assert_eq!(refusal.particulars, Vec::new());
    }
}
