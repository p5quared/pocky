use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{Mutex as TokioMutex, RwLock};

use crate::application::domain::{GameId, LobbyId, PlayerId};
use crate::application::ports::in_::{GameService, LobbyService, MatchmakingService};
use crate::application::ports::out_::{
    GameEventNotifier, GameEventScheduler, GameNotification, GameRepository, LobbyEventNotifier, LobbyNotification,
    LobbyRepository, MatchmakingEventNotifier, MatchmakingNotification, MatchmakingQueueRepository,
};

type WebSocketSender = SplitSink<WebSocket, Message>;

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum IncomingMessage {
    PlaceBid { game_id: GameId, value: i32 },
    PlaceAsk { game_id: GameId, value: i32 },
    JoinQueue,
    LeaveQueue,
    JoinLobby { lobby_id: LobbyId },
    LeaveLobby { lobby_id: LobbyId },
    Ready { lobby_id: LobbyId },
    Unready { lobby_id: LobbyId },
}

pub struct AppState<GN, GR, GS, MN, MR, LN, LR> {
    pub adapter: Arc<WebSocketAdapter>,
    pub game_service: Arc<TokioMutex<GameService<GN, GR, GS>>>,
    pub matchmaking_service: Arc<MatchmakingService<MN, MR>>,
    pub lobby_service: Arc<LobbyService<LN, LR>>,
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
        &self,
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

impl LobbyEventNotifier for WebSocketAdapter {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: LobbyNotification,
    ) {
        let message = serde_json::to_string(&notification).unwrap_or_default();
        self.send_to_player(player_id, &message).await;
    }
}

pub async fn handle_connection<GN, GR, GS, MN, MR, LN, LR>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<GN, GR, GS, MN, MR, LN, LR>>>,
) -> impl IntoResponse
where
    GN: GameEventNotifier + Send + 'static,
    GR: GameRepository + Send + 'static,
    GS: GameEventScheduler + Send + 'static,
    MN: MatchmakingEventNotifier + Send + Sync + 'static,
    MR: MatchmakingQueueRepository + Send + Sync + 'static,
    LN: LobbyEventNotifier + Send + Sync + 'static,
    LR: LobbyRepository + Send + Sync + 'static,
{
    ws.on_upgrade(move |socket| async move {
        let player_id = PlayerId::new();
        let (sender, receiver) = socket.split();

        state.adapter.register_player(player_id, sender).await;

        handle_messages(player_id, receiver, state).await;
    })
}

async fn handle_messages<GN, GR, GS, MN, MR, LN, LR>(
    player_id: PlayerId,
    mut receiver: SplitStream<WebSocket>,
    state: Arc<AppState<GN, GR, GS, MN, MR, LN, LR>>,
) where
    GN: GameEventNotifier + Send,
    GR: GameRepository + Send,
    GS: GameEventScheduler + Send,
    MN: MatchmakingEventNotifier + Send + Sync,
    MR: MatchmakingQueueRepository + Send + Sync,
    LN: LobbyEventNotifier + Send + Sync,
    LR: LobbyRepository + Send + Sync,
{
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(text) = message
            && let Ok(incoming) = serde_json::from_str::<IncomingMessage>(&text)
        {
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
                IncomingMessage::JoinLobby { lobby_id } => {
                    let _ = state.lobby_service.player_arrived(lobby_id, player_id).await;
                }
                IncomingMessage::LeaveLobby { lobby_id } => {
                    let _ = state.lobby_service.player_disconnected(lobby_id, player_id).await;
                }
                IncomingMessage::Ready { lobby_id } => {
                    let _ = state.lobby_service.player_ready(lobby_id, player_id).await;
                }
                IncomingMessage::Unready { lobby_id } => {
                    let _ = state.lobby_service.player_unready(lobby_id, player_id).await;
                }
            }
        }
    }

    // Handle disconnect - check if player was in a lobby
    if let Some(lobby_id) = state.lobby_service.find_lobby_by_player(player_id).await {
        let _ = state.lobby_service.player_disconnected(lobby_id, player_id).await;
    }

    state.adapter.unregister_player(player_id).await;
}
