use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::SinkExt;
use tokio::sync::{Mutex as TokioMutex, RwLock};
use tracing::debug;

use application::ports::out_::{GameEventNotifier, GameNotification, QueueNotifier};
use domain::{MatchmakingOutcome, PlayerId};

pub(crate) type WebSocketSender = SplitSink<WebSocket, Message>;

pub struct WebSocketNotifier {
    connections: RwLock<Vec<(PlayerId, TokioMutex<WebSocketSender>)>>,
}

impl WebSocketNotifier {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(Vec::new()),
        }
    }

    pub async fn register_player(&self, player_id: PlayerId, sender: WebSocketSender) {
        self.connections
            .write()
            .await
            .push((player_id, TokioMutex::new(sender)));
    }

    pub async fn unregister_player(&self, player_id: PlayerId) {
        self.connections
            .write()
            .await
            .retain(|(pid, _)| *pid != player_id);
    }

    async fn send_to_player(&self, player_id: PlayerId, message: &str) {
        debug!(player_id = ?player_id, message = %message, "-> Sending");
        let connections = self.connections.read().await;
        if let Some((_, sender)) = connections.iter().find(|(pid, _)| *pid == player_id) {
            let _ = sender
                .lock()
                .await
                .send(Message::Text(message.into()))
                .await;
        }
    }
}

impl Default for WebSocketNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GameEventNotifier for WebSocketNotifier {
    async fn notify_player(&self, player_id: PlayerId, notification: GameNotification) {
        let message = serde_json::to_string(&notification).unwrap_or_default();
        self.send_to_player(player_id, &message).await;
    }
}

#[async_trait]
impl QueueNotifier for WebSocketNotifier {
    async fn broadcast(&self, event: &MatchmakingOutcome) {
        let message = serde_json::to_string(event).unwrap_or_default();
        let connections = self.connections.read().await;
        for (player_id, sender) in connections.iter() {
            debug!(player_id = ?player_id, message = %message, "-> Broadcasting");
            let _ = sender
                .lock()
                .await
                .send(Message::Text(message.clone().into()))
                .await;
        }
    }
}
