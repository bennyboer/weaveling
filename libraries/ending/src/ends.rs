use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::extract::ws::CloseFrame;
use axum::http::request::Parts;
use live_contract::{SERVER_CLOSING, SESSION_ENDED};
use tokio_util::sync::CancellationToken;
use tokio_util::task::task_tracker::TaskTrackerToken;

use crate::{SessionEnd, Shutdown};

/// Why a socket has to end.
#[derive(Debug, PartialEq, Eq)]
pub enum End {
    SessionEnded,
    ServerClosing,
}

impl End {
    pub fn frame(&self) -> CloseFrame {
        match self {
            End::SessionEnded => CloseFrame {
                code: SESSION_ENDED,
                reason: "session-ended".into(),
            },
            End::ServerClosing => CloseFrame {
                code: SERVER_CLOSING,
                reason: "server-closing".into(),
            },
        }
    }
}

/// Everything a socket must end on, taken from its request: the session's `SessionEnd` and the server's `Shutdown`, each never reached where the request carries none. The server waits for the socket as long as its `Ends` is held.
pub struct Ends {
    session: SessionEnd,
    server: CancellationToken,
    _open: Option<TaskTrackerToken>,
}

impl Ends {
    pub async fn reached(&self) -> End {
        tokio::select! {
            () = self.session.reached() => End::SessionEnded,
            () = self.server.cancelled() => End::ServerClosing,
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Ends {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let shutdown = parts.extensions.get::<Shutdown>();

        Ok(Self {
            session: parts
                .extensions
                .get::<SessionEnd>()
                .cloned()
                .unwrap_or_else(SessionEnd::never),
            server: shutdown.map_or_else(CancellationToken::new, Shutdown::closing),
            _open: shutdown.map(Shutdown::stay),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use axum::http::Request;
    use tokio::sync::Notify;

    use super::*;
    use crate::Ending;

    const A_MOMENT: Duration = Duration::from_millis(50);

    #[derive(Default)]
    struct Signing(Notify);

    #[async_trait]
    impl Ending for Signing {
        async fn reached(&self) {
            self.0.notified().await;
        }
    }

    async fn ends_of(session: Option<SessionEnd>, shutdown: Option<&Shutdown>) -> Ends {
        let mut request = Request::builder();
        if let Some(session) = session {
            request = request.extension(session);
        }
        if let Some(shutdown) = shutdown {
            request = request.extension(shutdown.clone());
        }
        let (mut parts, ()) = request.body(()).expect("a request").into_parts();

        let Ok(ends) = Ends::from_request_parts(&mut parts, &()).await;
        ends
    }

    #[tokio::test]
    async fn the_session_coming_to_its_end_is_told_as_such() {
        // given: a request carrying a session end
        let signing = Arc::new(Signing::default());
        let ends = ends_of(Some(SessionEnd::of(signing.clone())), None).await;

        // when: the session comes to its end
        let reached = tokio::spawn(async move { ends.reached().await });
        tokio::time::sleep(A_MOMENT).await;
        signing.0.notify_waiters();

        // then: the end told is the session's
        assert_eq!(reached.await.expect("it ends"), End::SessionEnded);
    }

    #[tokio::test]
    async fn the_server_closing_is_told_as_such() {
        // given: a request carrying the server's shutdown
        let shutdown = Shutdown::new();
        let ends = ends_of(None, Some(&shutdown)).await;

        // when: the server closes
        let before = tokio::time::timeout(A_MOMENT, ends.reached()).await;
        shutdown.close();
        let after = tokio::time::timeout(A_MOMENT, ends.reached()).await;

        // then: nothing ended before
        assert!(
            before.is_err(),
            "nothing ends before the server is told to close"
        );
        // and: the end told after is the server's
        assert_eq!(after.ok(), Some(End::ServerClosing));
    }

    #[tokio::test]
    async fn a_request_carrying_neither_never_ends() {
        // given: a request carrying neither
        let ends = ends_of(None, None).await;

        // then: it never ends
        assert!(
            tokio::time::timeout(A_MOMENT, ends.reached())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn the_server_waits_until_the_last_socket_has_gone() {
        // given: two sockets of a closed server
        let shutdown = Shutdown::new();
        let first = ends_of(None, Some(&shutdown)).await;
        let second = ends_of(None, Some(&shutdown)).await;
        shutdown.close();

        // when: they go one by one
        drop(first);
        let waiting = tokio::time::timeout(A_MOMENT, shutdown.drained()).await;
        drop(second);
        let drained = tokio::time::timeout(A_MOMENT, shutdown.drained()).await;

        // then: the server waits while one is still open
        assert!(waiting.is_err(), "one socket is still open");
        // and: is drained once the last has gone
        assert!(drained.is_ok());
    }

    #[tokio::test]
    async fn the_server_is_not_drained_before_it_closes() {
        // given: a server not yet closed
        let shutdown = Shutdown::new();

        // then: it is not drained
        assert!(
            tokio::time::timeout(A_MOMENT, shutdown.drained())
                .await
                .is_err(),
            "a server with no socket open yet is still serving"
        );
    }
}
