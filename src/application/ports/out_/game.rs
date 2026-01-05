use std::time::Duration;

use serde::Serialize;

use crate::application::domain::{GameAction, GameError, GameEvent, GameId, GameState, PlayerId};

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

pub trait GameEventScheduler {
    fn schedule_action(
        &self,
        game_id: GameId,
        delay: Duration,
        action: GameAction,
    ) -> impl Future<Output = ()> + Send;
}
