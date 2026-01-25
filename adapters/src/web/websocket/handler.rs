use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::StreamExt;
use futures::stream::SplitStream;
use serde::Deserialize;
use tracing::{debug, info, warn};

use application::ports::in_::game_service;
use application::ports::in_::game_service::GameUseCase;
use domain::{GameId, MatchmakingOutcome, PlayerId};

use crate::web::state::AppState;

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
