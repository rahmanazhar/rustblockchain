use crate::dto::BlockDto;
use crate::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use rustchain_consensus::ConsensusEvent;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Global WebSocket connection counter (for metrics and limits).
static WS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

pub fn active_ws_connections() -> usize {
    WS_CONNECTIONS.load(Ordering::Relaxed)
}

/// Client-to-server subscription control messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsClientMessage {
    /// Subscribe to a named event channel.
    Subscribe { channel: SubscriptionChannel },
    /// Unsubscribe from a named event channel.
    Unsubscribe { channel: SubscriptionChannel },
    /// Ping to keep the connection alive.
    Ping,
}

/// Channels the client can subscribe to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionChannel {
    NewBlocks,
    NewTransactions,
    Finality,
}

/// Server-to-client push messages.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsServerMessage {
    NewBlock(BlockDto),
    TransactionPooled { tx_hash: String },
    BlockFinalized { height: u64 },
    Pong,
    Error { message: String },
}

/// Axum handler: upgrade an HTTP request to a WebSocket connection.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a single WebSocket connection.
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    WS_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
    info!(
        "WebSocket connected (active: {})",
        WS_CONNECTIONS.load(Ordering::Relaxed)
    );

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to consensus events.
    let mut event_rx: broadcast::Receiver<ConsensusEvent> = state.consensus.subscribe();

    // Track which channels this client cares about.
    let mut sub_blocks = false;
    let mut sub_txs = false;
    let mut sub_finality = false;

    loop {
        tokio::select! {
            // Incoming message from the client.
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<WsClientMessage>(&text) {
                            Ok(WsClientMessage::Subscribe { channel }) => {
                                match channel {
                                    SubscriptionChannel::NewBlocks => sub_blocks = true,
                                    SubscriptionChannel::NewTransactions => sub_txs = true,
                                    SubscriptionChannel::Finality => sub_finality = true,
                                }
                                debug!("Client subscribed to {:?}", channel);
                            }
                            Ok(WsClientMessage::Unsubscribe { channel }) => {
                                match channel {
                                    SubscriptionChannel::NewBlocks => sub_blocks = false,
                                    SubscriptionChannel::NewTransactions => sub_txs = false,
                                    SubscriptionChannel::Finality => sub_finality = false,
                                }
                                debug!("Client unsubscribed from {:?}", channel);
                            }
                            Ok(WsClientMessage::Ping) => {
                                let pong = serde_json::to_string(&WsServerMessage::Pong)
                                    .unwrap_or_default();
                                if sender.send(Message::Text(pong)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let err_msg = serde_json::to_string(&WsServerMessage::Error {
                                    message: format!("Invalid message: {}", e),
                                })
                                .unwrap_or_default();
                                if sender.send(Message::Text(err_msg)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        warn!("WebSocket receive error: {}", e);
                        break;
                    }
                    _ => {} // Binary, Ping, Pong frames -- ignore
                }
            }

            // Consensus event to push to the client.
            event = event_rx.recv() => {
                let server_msg = match event {
                    Ok(ConsensusEvent::NewBlock(block)) if sub_blocks => {
                        Some(WsServerMessage::NewBlock(BlockDto::from(block.as_ref())))
                    }
                    Ok(ConsensusEvent::TransactionPooled(hash)) if sub_txs => {
                        Some(WsServerMessage::TransactionPooled {
                            tx_hash: hash.to_string(),
                        })
                    }
                    Ok(ConsensusEvent::BlockFinalized(height)) if sub_finality => {
                        Some(WsServerMessage::BlockFinalized { height })
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WebSocket client lagged by {} events", n);
                        Some(WsServerMessage::Error {
                            message: format!("Lagged behind by {} events", n),
                        })
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => None,
                };

                if let Some(msg) = server_msg {
                    let text = serde_json::to_string(&msg).unwrap_or_default();
                    if sender.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    WS_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
    info!(
        "WebSocket disconnected (active: {})",
        WS_CONNECTIONS.load(Ordering::Relaxed)
    );
}
