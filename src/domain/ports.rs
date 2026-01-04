use std::time::Duration;

use serde::Serialize;

use super::types::LobbyId;
use super::{GameAction, GameError, GameEvent, GameState, LobbyEvent, LobbyState, PlayerId, types::GameId};

#[derive(Debug)]
pub enum GameServiceError {
    GameNotFound(GameId),
    GameError(GameError),
}

impl From<GameError> for GameServiceError {
    fn from(err: GameError) -> Self {
        GameServiceError::GameError(err)
    }
}

#[derive(Clone, Serialize)]
pub enum GameNotification {
    GameEvent(GameEvent),
}

pub enum MatchmakingServiceError {
    Foo, // TODO: Enumerate errors
}

#[derive(Clone, Serialize)]
pub enum MatchmakingNotification {
    PlayerJoinedQueue(PlayerId),
    PlayerLeftQueue(PlayerId),
    LobbyCreated(LobbyId),
}

#[derive(Debug)]
pub enum LobbyServiceError {
    LobbyNotFound(LobbyId),
    PlayerNotInLobby(PlayerId),
}

#[derive(Clone, Serialize)]
pub enum LobbyNotification {
    LobbyEvent(LobbyEvent),
    LobbyState {
        lobby_id: LobbyId,
        players: Vec<LobbyPlayerInfo>,
        phase: String,
        countdown_remaining: Option<u32>,
    },
}

#[derive(Clone, Serialize)]
pub struct LobbyPlayerInfo {
    pub player_id: PlayerId,
    pub is_ready: bool,
}

pub trait GameEventNotifier {
    fn notify_player(
        &self,
        player_id: PlayerId,
        notification: GameNotification,
    ) -> impl Future<Output = ()> + Send;
}

pub trait GameRepository {
    fn load_game(
        &self,
        game_id: GameId,
    ) -> impl Future<Output = Option<GameState>> + Send;
    fn save_game(
        &self,
        game_id: GameId,
        game_state: &GameState,
    ) -> impl Future<Output = ()> + Send;
}

pub trait AsyncTimer {
    fn sleep(
        &self,
        duration: Duration,
    ) -> impl Future<Output = ()> + Send;
}

pub trait GameEventScheduler {
    fn schedule_action(
        &self,
        game_id: GameId,
        delay: Duration,
        action: GameAction,
    ) -> impl Future<Output = ()> + Send;
}

pub trait MatchmakingQueueRepository {
    fn load_queue(&self) -> impl Future<Output = Vec<PlayerId>> + Send;
    fn save_queue(
        &self,
        queue: &Vec<PlayerId>,
    ) -> impl Future<Output = ()> + Send;
}

pub trait MatchmakingEventNotifier {
    fn notify_player(
        &self,
        player_id: PlayerId,
        notification: MatchmakingNotification,
    ) -> impl Future<Output = ()> + Send;
}

pub trait LobbyRepository {
    fn load_lobby(
        &self,
        lobby_id: LobbyId,
    ) -> impl Future<Output = Option<LobbyState>> + Send;

    fn save_lobby(
        &self,
        lobby_id: LobbyId,
        lobby_state: &LobbyState,
    ) -> impl Future<Output = ()> + Send;

    fn delete_lobby(
        &self,
        lobby_id: LobbyId,
    ) -> impl Future<Output = ()> + Send;

    fn find_lobby_by_player(
        &self,
        player_id: PlayerId,
    ) -> impl Future<Output = Option<LobbyId>> + Send;
}

pub trait LobbyEventNotifier {
    fn notify_player(
        &self,
        player_id: PlayerId,
        notification: LobbyNotification,
    ) -> impl Future<Output = ()> + Send;
}
