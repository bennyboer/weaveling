# ending

What tells a socket that it must end. There are two reasons: the session it was opened in has ended, or the server is stopping.

```rust
use ending::Ends;

async fn attach(upgrade: WebSocketUpgrade, ends: Ends) -> Response {
    upgrade.on_upgrade(move |socket| serve(socket, ends))
}

async fn serve(mut socket: WebSocket, ends: Ends) {
    let ended = ends.reached();
    tokio::pin!(ended);

    loop {
        tokio::select! {
            end = &mut ended => {
                _ = socket.send(Message::Close(Some(end.frame()))).await;
                break;
            }
            heard = socket.recv() => { /* … */ }
        }
    }
}
```

**A socket takes its `Ends`** as an extractor and selects on `reached()`. `reached()` returns why the socket ends: `End::SessionEnded` or `End::ServerClosing`. Each comes with its close frame from `live-contract`: `SESSION_ENDED` (4401) or `SERVER_CLOSING` (1001). As long as the socket holds its `Ends`, the server waits for it.

**When the session ends: `SessionEnd`.** The accounts gate adds it to the request, next to `Acting`. Its `reached()` returns once the session is over. A session is over when:

- the user signed out, here or somewhere else,
- the account was locked,
- the password was reset, or
- the session's last moment has come.

The accounts feature creates it, behind the `Ending` trait. So everything that holds a request open closes when the session ends, without knowing about accounts. Today that is the one live socket and the editing socket. If nothing adds a `SessionEnd` to the request, the session never ends.

**When the server stops: `Shutdown`.** `main` adds it to every route as an extension. On a signal, `main` calls `close()`. Then every socket's `Ends` is reached with `ServerClosing`. `drained()` returns once the last `Ends` is dropped. So the live scenes are settled only after nothing writes into them any more. `Shutdown` is built from a `CancellationToken` and a `TaskTracker` from `tokio-util`. If a request carries no `Shutdown`, the server never closes it.
