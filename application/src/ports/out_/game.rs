use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;

use domain::{GameAction, GameError, GameEvent, GameId, GameState, PlayerId};

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

#[async_trait]
pub trait GameEventNotifier: Send + Sync {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: GameNotification,
    );
}

#[async_trait]
pub trait GameRepository: Send + Sync {
    async fn load_game(
        &self,
        game_id: GameId,
    ) -> Option<GameState>;

    async fn save_game(
        &self,
        game_id: GameId,
        game_state: &GameState,
    );
}

#[async_trait]
pub trait GameEventScheduler: Send + Sync {
    async fn schedule_action(
        &self,
        game_id: GameId,
        delay: Duration,
        action: GameAction,
    );
}
