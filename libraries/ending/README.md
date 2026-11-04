# ending

What tells a socket it must end: the session it was opened in coming to its end, or the server stopping.

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

**A socket takes its `Ends`** as an extractor and selects on `reached()`, which tells why it ends: `End::SessionEnded` or `End::ServerClosing`, each with its close frame, `SESSION_ENDED` (4401) or `SERVER_CLOSING` (1001) from `live-contract`. As long as the socket holds its `Ends`, the server waits for it.

**When the session ends, `SessionEnd`.** Accounts' gate puts it into the request beside `Acting`: `reached()` returns once the session is over, signed out here or elsewhere, its account locked, its password reset, or its last moment come; accounts make it, behind the `Ending` trait, so whatever holds a request open, the one live socket and the editing socket, closes on it without knowing accounts. Where nothing puts one there, the session never ends.

**When the server stops, `Shutdown`.** `main` puts it into every route as an extension. On a signal `main` calls `close()`, every socket's `Ends` is reached as `ServerClosing`, and `drained()` returns once the last `Ends` is dropped, so the live scenes are settled only after nothing writes into them any more. It is a `CancellationToken` and a `TaskTracker` from `tokio-util`; where a request carries no `Shutdown`, the server never closes it.
