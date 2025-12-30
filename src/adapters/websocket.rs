use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{Mutex as TokioMutex, RwLock};

use crate::domain::ports::{
    GameEventNotifier, GameNotification, GameRepository, MatchmakingEventNotifier,
    MatchmakingNotification, MatchmakingQueueRepository,
};
use crate::domain::services::{GameService, MatchmakingService};
use crate::domain::{GameId, PlayerId};

type WebSocketSender = SplitSink<WebSocket, Message>;

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum IncomingMessage {
    PlaceBid { game_id: GameId, value: i32 },
    PlaceAsk { game_id: GameId, value: i32 },
    JoinQueue,
    LeaveQueue,
}

pub struct AppState<GN, GR, MN, MR> {
    pub adapter: Arc<WebSocketAdapter>,
    pub game_service: Arc<TokioMutex<GameService<GN, GR>>>,
    pub matchmaking_service: Arc<MatchmakingService<MN, MR>>,
}

pub struct WebSocketAdapter {
    connections: RwLock<Vec<(PlayerId, TokioMutex<WebSocketSender>)>>,
}

impl WebSocketAdapter {
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
        let connections = self.connections.read().await;
        if let Some((_, sender)) = connections.iter().find(|(pid, _)| *pid == player_id) {
            let _ = sender.lock().await.send(Message::Text(message.into())).await;
        }
    }
}

impl Default for WebSocketAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GameEventNotifier for WebSocketAdapter {
    async fn notify_player(
        &mut self,
        player_id: PlayerId,
        notification: GameNotification,
    ) {
        let message = serde_json::to_string(&notification).unwrap_or_default();
        self.send_to_player(player_id, &message).await;
    }
}

impl MatchmakingEventNotifier for WebSocketAdapter {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: MatchmakingNotification,
    ) {
        let message = serde_json::to_string(&notification).unwrap_or_default();
        self.send_to_player(player_id, &message).await;
    }
}

pub async fn handle_connection<GN, GR, MN, MR>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<GN, GR, MN, MR>>>,
) -> impl IntoResponse
where
    GN: GameEventNotifier + Send + 'static,
    GR: GameRepository + Send + 'static,
    MN: MatchmakingEventNotifier + Send + Sync + 'static,
    MR: MatchmakingQueueRepository + Send + Sync + 'static,
{
    ws.on_upgrade(move |socket| async move {
        let player_id = PlayerId::new();
        let (sender, receiver) = socket.split();

        state.adapter.register_player(player_id, sender).await;

        handle_messages(player_id, receiver, state).await;
    })
}

async fn handle_messages<GN, GR, MN, MR>(
    player_id: PlayerId,
    mut receiver: SplitStream<WebSocket>,
    state: Arc<AppState<GN, GR, MN, MR>>,
) where
    GN: GameEventNotifier + Send,
    GR: GameRepository + Send,
    MN: MatchmakingEventNotifier + Send + Sync,
    MR: MatchmakingQueueRepository + Send + Sync,
{
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(text) = message {
            if let Ok(incoming) = serde_json::from_str::<IncomingMessage>(&text) {
                match incoming {
                    IncomingMessage::PlaceBid { game_id, value } => {
                        let _ = state.game_service.lock().await.place_bid(game_id, player_id, value).await;
                    }
                    IncomingMessage::PlaceAsk { game_id, value } => {
                        let _ = state.game_service.lock().await.place_ask(game_id, player_id, value).await;
                    }
                    IncomingMessage::JoinQueue => {
                        let _ = state.matchmaking_service.join_queue(player_id).await;
                    }
                    IncomingMessage::LeaveQueue => {
                        let _ = state.matchmaking_service.leave_queue(player_id).await;
                    }
                }
            }
        }
    }

    state.adapter.unregister_player(player_id).await;
}
