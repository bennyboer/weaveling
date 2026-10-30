# live-contract

What one frame on a live channel looks like.

`FrameDTO` is what a frame is `of`, its `version`, when it happened, `at`, if it was published, and the `event`. On the one socket, `/live`, the client asks with `AskedDTO` (`subscribe` with an id, a topic and what it is `about`; `unsubscribe`) and is told with `ToldDTO` (a `frame`, a `FrameDTO`; a `notice`, `ACCESS_CHANGED`; or `ended` with why, one of `ended`'s; each under the subscription's id). A socket whose session ends is closed with `SESSION_ENDED`. The server's [`live`](../live) writes it and its sieves read it, and the client reads it, so the shape is written once and cannot drift apart.

The client subscribes to a project's ideas and reads what it is told:

```rust
use std::collections::BTreeMap;

use live_contract::{AskedDTO, ToldDTO};

let asked = AskedDTO::Subscribe {
    id: "ideas".to_owned(),
    topic: "ideas".to_owned(),
    about: BTreeMap::from([("project".to_owned(), "project_1".to_owned())]),
};
socket.send(&serde_json::to_string(&asked)?);

match serde_json::from_str::<ToldDTO>(&heard)? {
    ToldDTO::Frame { id, frame } => taken_in(&id, frame),
    ToldDTO::Notice { id, notice } => noticed(&id, &notice),
    ToldDTO::Ended { id, why } => ended(&id, &why),
}
```

**A crate of its own so the client can depend on it** without depending on `live`, which brings axum, tokio and the sockets. The same reason every feature has a `contract`, and `messaging` its [`messaging-contract`](../messaging-contract).
