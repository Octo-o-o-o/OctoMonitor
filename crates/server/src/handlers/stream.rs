use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};

use tokio::sync::broadcast::error::RecvError;

use crate::state::AppState;

pub async fn stream(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_socket(socket, state))
}

async fn stream_socket(mut socket: WebSocket, state: AppState) {
    if send_snapshot(&mut socket, &state).await.is_err() {
        return;
    }

    let mut rx = state.notify.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) | Ok(_) => {}
                }
                // Drain backlog so we only resend the latest snapshot.
                while rx.try_recv().is_ok() {}

                if send_snapshot(&mut socket, &state).await.is_err() {
                    break;
                }
            }
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}

async fn send_snapshot(socket: &mut WebSocket, state: &AppState) -> Result<(), axum::Error> {
    let payload = state.bootstrap.read().await.clone();
    let msg = serde_json::json!({"type": "snapshot.replace", "payload": payload});
    socket.send(Message::Text(msg.to_string().into())).await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::Router;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message as ClientMessage;

    use crate::{build_app, pricing::PricingStore, probe::empty_bootstrap, state::AppState};

    /// Spin up the real Axum router on `127.0.0.1:0` so we can drive the WS
    /// upgrade through a real `tokio-tungstenite` client. This is the only
    /// way to exercise the upgrade path — `Router::oneshot` cannot upgrade.
    /// `JoinHandle::abort()` on drop keeps the listener task from outliving
    /// the test and bleeding into subsequent runs.
    struct WsHarness {
        state: AppState,
        url: String,
        server: tokio::task::JoinHandle<()>,
    }

    impl WsHarness {
        async fn start() -> Self {
            let state = AppState::new(empty_bootstrap(), PricingStore::new());
            let app: Router = build_app(state.clone());
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind ephemeral port");
            let addr = listener.local_addr().expect("local addr");
            let server = tokio::spawn(async move {
                let _ = axum::serve(listener, app.into_make_service()).await;
            });
            Self {
                state,
                url: format!("ws://{addr}/api/stream"),
                server,
            }
        }
    }

    impl Drop for WsHarness {
        fn drop(&mut self) {
            self.server.abort();
        }
    }

    async fn next_text(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> String {
        let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("ws frame within timeout")
            .expect("stream not closed")
            .expect("ok frame");
        match frame {
            ClientMessage::Text(text) => text.to_string(),
            other => panic!("expected text frame, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn stream_sends_initial_snapshot_then_resends_on_signal_change() {
        let harness = WsHarness::start().await;
        let (mut socket, _resp) = tokio_tungstenite::connect_async(&harness.url)
            .await
            .expect("ws connect");

        // First frame: the initial snapshot pushed on upgrade.
        let initial = next_text(&mut socket).await;
        let payload: serde_json::Value = serde_json::from_str(&initial).unwrap();
        assert_eq!(
            payload.get("type").and_then(|v| v.as_str()),
            Some("snapshot.replace")
        );
        assert!(
            payload.get("payload").is_some(),
            "snapshot must carry a payload"
        );

        // Mutate state and signal — the connection should receive another
        // snapshot.replace covering the change.
        {
            let mut bootstrap = harness.state.bootstrap.write().await;
            bootstrap.config.history_days = 7;
        }
        harness.state.signal_change();

        let second = next_text(&mut socket).await;
        let payload: serde_json::Value = serde_json::from_str(&second).unwrap();
        assert_eq!(
            payload.get("type").and_then(|v| v.as_str()),
            Some("snapshot.replace")
        );
        assert_eq!(
            payload
                .get("payload")
                .and_then(|p| p.get("config"))
                .and_then(|c| c.get("historyDays"))
                .and_then(|v| v.as_u64()),
            Some(7)
        );

        // Tidy: close the client so the server-side task exits its loop.
        let _ = socket.send(ClientMessage::Close(None)).await;
    }
}
