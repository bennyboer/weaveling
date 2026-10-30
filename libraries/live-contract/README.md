# live-contract

What one frame on a live channel looks like.

`FrameDTO` is what a frame is `of`, its `version`, when it happened, `at`, if it was published, and the `event`. On the one socket, `/live`, the client asks with `AskedDTO` (`subscribe` with an id, a topic and what it is `about`; `unsubscribe`) and is told with `ToldDTO` (`frame`, `notice` or `ended` with why, each under the subscription's id). The server's [`live`](../live) writes it and its sieves read it, and the client reads it, so the shape is written once and cannot drift apart.

**A crate of its own so the client can depend on it** without depending on `live`, which brings axum, tokio and the sockets. The same reason every feature has a `contract`, and `messaging` its [`messaging-contract`](../messaging-contract).
