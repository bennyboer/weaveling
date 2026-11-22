use std::collections::BTreeMap;

use live_contract::FrameDTO;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(raw_module = "/js/shell.js")]
extern "C" {
    type LiveSubscription;

    #[wasm_bindgen(constructor)]
    fn new(
        topic: &str,
        about: &str,
        heard: &Closure<dyn Fn(String)>,
        opened: &Closure<dyn Fn()>,
        not_there: &JsValue,
    ) -> LiveSubscription;

    #[wasm_bindgen(method)]
    fn close(this: &LiveSubscription);

    #[wasm_bindgen(js_name = whenSessionEnded)]
    fn when_session_ended_told(told: &Closure<dyn Fn()>);
}

pub fn when_session_ended(told: impl Fn() + 'static) {
    let told = Closure::<dyn Fn()>::new(told);
    when_session_ended_told(&told);
    told.forget();
}

fn heard_as_frames(heard: impl Fn(FrameDTO) + 'static) -> Closure<dyn Fn(String)> {
    Closure::<dyn Fn(String)>::new(move |frame: String| {
        if let Ok(told) = serde_json::from_str::<FrameDTO>(&frame) {
            heard(told);
        }
    })
}

fn about_as_told(about: &[(&str, &str)]) -> String {
    serde_json::to_string(&about.iter().copied().collect::<BTreeMap<&str, &str>>())
        .expect("what a subscription is about is plain text")
}

pub struct Subscription {
    subscription: LiveSubscription,
    _heard: Closure<dyn Fn(String)>,
    _opened: Closure<dyn Fn()>,
    _not_there: Option<Closure<dyn Fn()>>,
}

impl Subscription {
    pub fn start(
        topic: &str,
        about: &[(&str, &str)],
        heard: impl Fn(FrameDTO) + 'static,
        opened: impl Fn() + 'static,
    ) -> Self {
        Self::started(topic, about, heard, opened, None)
    }

    /// Starts one that is also told the first time the server finds nothing there to follow, while it keeps asking again.
    /// The server says so alike for what is not there and for what the asker may not follow,
    /// so whoever started it can ask what is there by other means.
    pub fn start_told_not_there(
        topic: &str,
        about: &[(&str, &str)],
        heard: impl Fn(FrameDTO) + 'static,
        opened: impl Fn() + 'static,
        not_there: impl Fn() + 'static,
    ) -> Self {
        Self::started(
            topic,
            about,
            heard,
            opened,
            Some(Closure::<dyn Fn()>::new(not_there)),
        )
    }

    fn started(
        topic: &str,
        about: &[(&str, &str)],
        heard: impl Fn(FrameDTO) + 'static,
        opened: impl Fn() + 'static,
        not_there: Option<Closure<dyn Fn()>>,
    ) -> Self {
        let heard = heard_as_frames(heard);
        let opened = Closure::<dyn Fn()>::new(opened);
        let subscription = LiveSubscription::new(
            topic,
            &about_as_told(about),
            &heard,
            &opened,
            not_there
                .as_ref()
                .map_or(&JsValue::UNDEFINED, |told| told.as_ref()),
        );

        Self {
            subscription,
            _heard: heard,
            _opened: opened,
            _not_there: not_there,
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.subscription.close();
    }
}

#[cfg(test)]
mod tests {
    use boards_contract::BoardEventDTO;

    use super::*;

    #[test]
    fn a_frame_as_the_server_sends_it_is_read_with_its_event() {
        // when: a frame as the server sends it is read
        let told: FrameDTO = serde_json::from_str(r#"{"event":{"at":{"x":400,"y":200},"idea":"idea_1","name":"IDEA_PINNED","size":{"height":84,"width":168}},"version":2}"#).expect("a frame");

        // then: it carries its version and an event its feature can read
        assert_eq!(told.version, 2);
        assert!(serde_json::from_value::<BoardEventDTO>(told.event).is_ok());
    }

    #[test]
    fn what_a_subscription_is_about_is_told_as_one_object() {
        // expect: each thing it is about is a field of one object
        assert_eq!(
            about_as_told(&[("mine", "true"), ("project", "project_1")]),
            r#"{"mine":"true","project":"project_1"}"#
        );
        // and: about nothing is an empty one
        assert_eq!(about_as_told(&[]), "{}");
    }
}
