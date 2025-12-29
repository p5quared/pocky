use std::time::Duration;

use super::{GameEvent, GameState, PlayerId, types::GameId};

#[derive(Debug)]
pub enum GameServiceError {
    GameNotFound(GameId),
}

pub enum GameNotification {
    GameEvent(GameEvent),
}

pub enum MatchmakingServiceError {
    Foo, // TODO: Enumerate errors
}

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
    ) -> impl Future<Output = ()>;
}

pub trait GameRepository {
    fn load_game(
        &self,
        game_id: GameId,
    ) -> impl Future<Output = Option<GameState>>;
    fn save_game(
        &self,
        game_id: GameId,
        game_state: &GameState,
    ) -> impl Future<Output = ()>;
}

pub trait AsyncTimer {
    fn sleep(
        &self,
        duration: Duration,
    ) -> impl Future<Output = ()>;
}

pub trait MatchmakingQueueRepository {
    fn load_queue(&self) -> impl Future<Output = Vec<PlayerId>>;
    fn save_queue(
        &self,
        queue: &Vec<PlayerId>,
    ) -> impl Future<Output = ()>;
}

pub trait MatchmakingEventNotifier {
    fn notify_player(
        &self,
        player_id: PlayerId,
        notification: MatchmakingNotification,
    ) -> impl Future<Output = ()>;
}
