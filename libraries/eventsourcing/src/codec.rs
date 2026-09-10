use serde_json::Value;

pub struct Codec<E> {
    pub body: fn(&E) -> Value,
    pub event: fn(Value) -> Option<E>,
}
