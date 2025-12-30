use std::time::Duration;

use serde::Serialize;

use super::{GameEvent, GameState, PlayerId, types::GameId};

#[derive(Debug)]
pub enum GameServiceError {
    GameNotFound(GameId),
}

#[derive(Serialize)]
pub enum GameNotification {
    GameEvent(GameEvent),
}

pub enum MatchmakingServiceError {
    Foo, // TODO: Enumerate errors
}

#[derive(Serialize)]
pub enum MatchmakingNotification {
    PlayerJoinedQueue(PlayerId),
    PlayerLeftQueue(PlayerId),
    GameFound(GameId),
}

pub trait GameEventNotifier {
    fn notify_player(
        &mut self,
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
