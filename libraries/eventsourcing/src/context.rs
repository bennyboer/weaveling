use std::collections::BTreeMap;

use serde_json::{Map, Value};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AggregateContext(BTreeMap<String, String>);

impl AggregateContext {
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.0.insert(key.to_owned(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn entries(&self) -> &BTreeMap<String, String> {
        &self.0
    }

    pub fn written(&self) -> Value {
        Value::Object(
            self.0
                .iter()
                .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                .collect::<Map<String, Value>>(),
        )
    }

    pub fn read(written: &Value) -> Option<Self> {
        written
            .as_object()?
            .iter()
            .map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
            .collect::<Option<BTreeMap<_, _>>>()
            .map(Self)
    }
}

impl From<BTreeMap<String, String>> for AggregateContext {
    fn from(entries: BTreeMap<String, String>) -> Self {
        Self(entries)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_context_holds_what_it_was_given_by_name() {
        let context = AggregateContext::default()
            .with("shelf", "shelf_1")
            .with("room", "room_2");

        assert_eq!(context.get("shelf"), Some("shelf_1"));
        assert_eq!(context.get("room"), Some("room_2"));
        assert_eq!(context.get("drawer"), None);
        assert!(!context.is_empty());
        assert!(AggregateContext::default().is_empty());
    }

    #[test]
    fn a_context_is_written_as_an_object_and_read_back_as_it_was() {
        let context = AggregateContext::default().with("shelf", "shelf_1");

        assert_eq!(context.written(), json!({ "shelf": "shelf_1" }));
        assert_eq!(
            AggregateContext::read(&context.written()),
            Some(context.clone())
        );
        assert_eq!(
            AggregateContext::read(&json!({})),
            Some(AggregateContext::default())
        );
    }

    #[test]
    fn what_is_no_context_reads_as_nothing() {
        for written in [
            json!("shelf_1"),
            json!({ "shelf": 1 }),
            json!(["shelf", "shelf_1"]),
        ] {
            assert_eq!(AggregateContext::read(&written), None, "{written}");
        }
    }
}
