# live-contract

What one frame on a live channel looks like.

A `FrameDTO` holds:

- what the frame is `of`,
- its `version`,
- `at`: when it happened, if it was published,
- the `event`.

There is one socket, `/live`. The client sends `AskedDTO` messages:

- `subscribe`, with an id, a topic and what it is `about`,
- `unsubscribe`.

The server answers with `ToldDTO` messages, each under the subscription's id:

- a `frame`, which holds a `FrameDTO`,
- a `notice`, such as `ACCESS_CHANGED`,
- `ended`, with the reason why. The reason is one of the values in `ended`.

A socket whose session ends is closed with `SESSION_ENDED`. The server's [`live`](../live) writes these shapes, and its sieves read them. The client reads them too. So the shape is defined once and the two sides cannot drift apart.

The client subscribes to a project's ideas and reads what it receives:

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

**It is a crate of its own, so the client can depend on it** without depending on `live`. `live` brings in axum, tokio and the sockets. For the same reason every feature has a `contract`, and `messaging` has its [`messaging-contract`](../messaging-contract).
