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
    // Send initial snapshot immediately on connect
    {
        let payload = state.bootstrap.read().await.clone();
        let msg = serde_json::json!({"type": "snapshot.replace", "payload": payload});
        if socket
            .send(Message::Text(msg.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    // Subscribe to state change notifications
    let mut rx = state.notify.subscribe();

    loop {
        tokio::select! {
            // State changed — send new snapshot
            result = rx.recv() => {
                match result {
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => {} // skip, send fresh snapshot below
                    Ok(_) => {}
                }
                // Drain any extra signals that arrived while we were sending
                while rx.try_recv().is_ok() {}

                let payload = state.bootstrap.read().await.clone();
                let msg = serde_json::json!({"type": "snapshot.replace", "payload": payload});
                if socket
                    .send(Message::Text(msg.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            // Client sent something (ping/pong/close) or disconnected
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {} // ignore text/binary from client
                }
            }
        }
    }
}
