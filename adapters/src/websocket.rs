use std::collections::HashMap;
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

use application::ports::in_::game_service::{GameStore, GameUseCase};
use application::ports::in_::{MatchmakingService, game_service};
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
    CancelBid { game_id: GameId, price: i32 },
    CancelAsk { game_id: GameId, price: i32 },
}

pub struct AppState {
    pub notifier: Arc<WebSocketNotifier>,
    pub game_store: GameStore,
    pub matchmaking_service: Arc<TokioMutex<MatchmakingService>>,
}

impl AppState {
    pub fn new(
        notifier: Arc<WebSocketNotifier>,
        game_store: GameStore,
        matchmaking_service: Arc<TokioMutex<MatchmakingService>>,
    ) -> Self {
        Self {
            notifier,
            game_store,
            matchmaking_service,
        }
    }
}

pub fn create_app_state() -> Arc<AppState> {
    let notifier = Arc::new(WebSocketNotifier::new());
    let game_store = Arc::new(RwLock::new(HashMap::new()));
    let queue_notifier: Arc<dyn QueueNotifier> = notifier.clone();
    let matchmaking_service = MatchmakingService::new(queue_notifier);

    Arc::new(AppState::new(
        notifier,
        game_store,
        Arc::new(TokioMutex::new(matchmaking_service)),
    ))
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
    async fn broadcast(&self, event: &MatchmakingOutcome) {
        let message = serde_json::to_string(event).unwrap_or_default();
        let connections = self.connections.read().await;
        for (player_id, sender) in connections.iter() {
            debug!(player_id = ?player_id, message = %message, "-> Broadcasting");
            let _ = sender.lock().await.send(Message::Text(message.clone().into())).await;
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
        state.notifier.register_player(player_id, sender).await;

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
                        let _ = game_service::execute(
                            Arc::clone(&state.notifier),
                            Arc::clone(&state.game_store),
                            GameUseCase::PlaceBid {
                                game_id,
                                player_id,
                                value,
                            },
                        )
                        .await;
                    }
                    IncomingMessage::PlaceAsk { game_id, value } => {
                        let _ = game_service::execute(
                            Arc::clone(&state.notifier),
                            Arc::clone(&state.game_store),
                            GameUseCase::PlaceAsk {
                                game_id,
                                player_id,
                                value,
                            },
                        )
                        .await;
                    }
                    IncomingMessage::CancelBid { game_id, price } => {
                        let _ = game_service::execute(
                            Arc::clone(&state.notifier),
                            Arc::clone(&state.game_store),
                            GameUseCase::CancelBid {
                                game_id,
                                player_id,
                                price,
                            },
                        )
                        .await;
                    }
                    IncomingMessage::CancelAsk { game_id, price } => {
                        let _ = game_service::execute(
                            Arc::clone(&state.notifier),
                            Arc::clone(&state.game_store),
                            GameUseCase::CancelAsk {
                                game_id,
                                player_id,
                                price,
                            },
                        )
                        .await;
                    }
                    IncomingMessage::JoinQueue => {
                        let mut matchmaking_s = state.matchmaking_service.lock().await;
                        let outcome = matchmaking_s.join_queue(player_id).await;
                        if let MatchmakingOutcome::Matched(players) = outcome {
                            let _ = game_service::execute(
                                Arc::clone(&state.notifier),
                                Arc::clone(&state.game_store),
                                GameUseCase::LaunchGame {
                                    players,
                                    config: domain::GameConfig::default(),
                                },
                            )
                            .await;
                        } else {
                            debug!(player_id = ?player_id, event = ?outcome, "Player joined queue");
                        }
                    }
                    IncomingMessage::LeaveQueue => {
                        state.matchmaking_service.lock().await.remove_player(player_id).await;
                    }
                },
                Err(e) => {
                    warn!(player_id = ?player_id, error = %e, "Failed to parse message");
                }
            }
        }
    }

    info!(player_id = ?player_id, "Player disconnected");
    state.notifier.unregister_player(player_id).await;
}
