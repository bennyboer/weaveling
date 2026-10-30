use std::collections::BTreeMap;

use live_contract::FrameDTO;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(module = "/js/editor.bundle.js")]
extern "C" {
    type LiveFollowing;

    #[wasm_bindgen(constructor)]
    fn new(
        path: &str,
        heard: &Closure<dyn Fn(String)>,
        opened: &Closure<dyn Fn()>,
    ) -> LiveFollowing;

    #[wasm_bindgen(method)]
    fn close(this: &LiveFollowing);

    type LiveSubscription;

    #[wasm_bindgen(constructor)]
    fn new(
        topic: &str,
        about: &str,
        heard: &Closure<dyn Fn(String)>,
        opened: &Closure<dyn Fn()>,
    ) -> LiveSubscription;

    #[wasm_bindgen(method)]
    fn close(this: &LiveSubscription);
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
}

impl Subscription {
    pub fn start(
        topic: &str,
        about: &[(&str, &str)],
        heard: impl Fn(FrameDTO) + 'static,
        opened: impl Fn() + 'static,
    ) -> Self {
        let heard = heard_as_frames(heard);
        let opened = Closure::<dyn Fn()>::new(opened);
        let subscription = LiveSubscription::new(topic, &about_as_told(about), &heard, &opened);

        Self {
            subscription,
            _heard: heard,
            _opened: opened,
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.subscription.close();
    }
}

pub struct Following {
    connection: LiveFollowing,
    _heard: Closure<dyn Fn(String)>,
    _opened: Closure<dyn Fn()>,
}

impl Following {
    pub fn start(
        path: &str,
        heard: impl Fn(FrameDTO) + 'static,
        opened: impl Fn() + 'static,
    ) -> Self {
        let heard = heard_as_frames(heard);
        let opened = Closure::<dyn Fn()>::new(opened);
        let connection = LiveFollowing::new(path, &heard, &opened);

        Self {
            connection,
            _heard: heard,
            _opened: opened,
        }
    }
}

impl Drop for Following {
    fn drop(&mut self) {
        self.connection.close();
    }
}

#[cfg(test)]
mod tests {
    use boards_contract::BoardEventDTO;

    use super::*;

    #[test]
    fn a_frame_as_the_server_sends_it_is_read_with_its_event() {
        let told: FrameDTO = serde_json::from_str(r#"{"event":{"at":{"x":400,"y":200},"idea":"idea_1","name":"IDEA_PINNED","size":{"height":84,"width":168}},"version":2}"#).expect("a frame");

        assert_eq!(told.version, 2);
        assert!(serde_json::from_value::<BoardEventDTO>(told.event).is_ok());
    }

    #[test]
    fn what_a_subscription_is_about_is_told_as_one_object() {
        assert_eq!(
            about_as_told(&[("mine", "true"), ("project", "project_1")]),
            r#"{"mine":"true","project":"project_1"}"#
        );
        assert_eq!(about_as_told(&[]), "{}");
    }
}
