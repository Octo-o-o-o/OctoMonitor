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
