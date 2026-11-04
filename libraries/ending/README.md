# ending

What tells a socket it must end: the session it was opened in coming to its end, or the server stopping.

```rust
use ending::{Closing, SessionEnd};

async fn serve(socket: WebSocket, session: SessionEnd, closing: Option<Closing>) {
    let _staying = closing.as_ref().map(Closing::stay);
    let server_closing = async {
        match &closing {
            Some(closing) => closing.closed().await,
            None => std::future::pending().await,
        }
    };

    tokio::select! {
        () = session.reached() => { /* close as SESSION_ENDED */ }
        () = server_closing => { /* close as SERVER_CLOSING */ }
    }
}
```

**When the session ends, `SessionEnd`.** Accounts' gate puts it into the request beside `Acting`: `reached()` returns once the session is over, signed out here or elsewhere, its account locked, its password reset, or its last moment come; accounts make it, behind the `Ending` trait, so whatever holds a request open, the one live socket and the editing socket, closes on it without knowing accounts. Where nothing puts one there, `SessionEnd::never()` never ends.

**When the server stops, `Shutdown` and `Closing`.** *Added 2026-11-04.* `main` keeps the `Shutdown` and hands every route its `Closing` as an extension. A socket takes a `Staying` when it is upgraded and holds it as long as it runs, and listens for `closed()`. On a signal `main` calls `close()`, every socket stops taking work and closes, and `drained()` returns once the last `Staying` is dropped, so the live scenes are settled only after nothing writes into them any more. Holding a `Closing` is not staying: the router holds one for good, and `drained()` does not wait for it.
