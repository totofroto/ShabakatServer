use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use tokio::sync::broadcast;

use crate::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.broadcast_tx.clone()))
}

async fn handle_socket(mut socket: WebSocket, tx: broadcast::Sender<serde_json::Value>) {
    let mut rx = tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(msg) => {
                let text = match serde_json::to_string(&msg) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }
}
