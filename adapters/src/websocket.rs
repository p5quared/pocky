use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{Mutex as TokioMutex, RwLock};
use tracing::{debug, info, warn};

use application::ports::in_::{GameService, MatchmakingService};
use application::ports::out_::{GameEventNotifier, GameNotification, QueueNotifier};
use domain::{GameId, MatchmakingOutcome, PlayerId};

type WebSocketSender = SplitSink<WebSocket, Message>;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IncomingMessage {
    JoinQueue,
    LeaveQueue,
    PlaceBid { game_id: GameId, value: i32 },
    PlaceAsk { game_id: GameId, value: i32 },
}

pub struct AppState {
    pub ws_messenger: Arc<WebSocketNotifier>,
    pub game_service: Arc<TokioMutex<GameService>>,
    pub matchamaking_service: Arc<TokioMutex<MatchmakingService>>,
}

impl AppState {
    pub fn new(
        adapter: Arc<WebSocketNotifier>,
        game_service: Arc<TokioMutex<GameService>>,
        matchamaking_service: Arc<TokioMutex<MatchmakingService>>,
    ) -> Self {
        Self {
            ws_messenger: adapter,
            game_service,
            matchamaking_service,
        }
    }
}

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

    pub async fn register_player(
        &self,
        player_id: PlayerId,
        sender: WebSocketSender,
    ) {
        self.connections.write().await.push((player_id, TokioMutex::new(sender)));
    }

    pub async fn unregister_player(
        &self,
        player_id: PlayerId,
    ) {
        self.connections.write().await.retain(|(pid, _)| *pid != player_id);
    }

    async fn send_to_player(
        &self,
        player_id: PlayerId,
        message: &str,
    ) {
        debug!(player_id = ?player_id, message = %message, "-> Sending");
        let connections = self.connections.read().await;
        if let Some((_, sender)) = connections.iter().find(|(pid, _)| *pid == player_id) {
            let _ = sender.lock().await.send(Message::Text(message.into())).await;
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
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: GameNotification,
    ) {
        let message = serde_json::to_string(&notification).unwrap_or_default();
        self.send_to_player(player_id, &message).await;
    }
}

#[async_trait]
impl QueueNotifier for WebSocketNotifier {
    async fn broadcast(
        &self,
        players: &[PlayerId],
        event: &MatchmakingOutcome,
    ) {
        let message = serde_json::to_string(event).unwrap_or_default();
        for player_id in players {
            self.send_to_player(*player_id, &message).await;
        }
    }
}

pub async fn handle_connection(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let player_id = PlayerId::new();
        info!(player_id = ?player_id, "Player connected");

        let (sender, receiver) = socket.split();
        state.ws_messenger.register_player(player_id, sender).await;

        handle_messages(player_id, receiver, state).await;
    })
}

async fn handle_messages(
    player_id: PlayerId,
    mut receiver: SplitStream<WebSocket>,
    state: Arc<AppState>,
) {
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(text) = message {
            debug!(player_id = ?player_id, message = %text, "<- Received");

            match serde_json::from_str::<IncomingMessage>(&text) {
                Ok(incoming) => match incoming {
                    IncomingMessage::PlaceBid { game_id, value } => {
                        let _ = state.game_service.lock().await.place_bid(game_id, player_id, value).await;
                    }
                    IncomingMessage::PlaceAsk { game_id, value } => {
                        let _ = state.game_service.lock().await.place_ask(game_id, player_id, value).await;
                    }
                    IncomingMessage::JoinQueue => {
                        let mut matchmaking_s = state.matchamaking_service.lock().await;
                        let game_s = state.game_service.lock().await;
                        let outcome = matchmaking_s.join_queue(player_id).await;
                        match outcome {
                            MatchmakingOutcome::Matched(players) => {
                                let _ = game_s.launch_game(players, domain::GameConfig::default()).await;
                            }
                            e => {
                                debug!(player_id = ?player_id, event = ?e, "Player joined queue");
                            }
                        }
                    }
                    IncomingMessage::LeaveQueue => {
                        state.matchamaking_service.lock().await.remove_player(player_id).await;
                    }
                },
                Err(e) => {
                    warn!(player_id = ?player_id, error = %e, "Failed to parse message");
                }
            }
        }
    }

    info!(player_id = ?player_id, "Player disconnected");
    state.ws_messenger.unregister_player(player_id).await;
}
