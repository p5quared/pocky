use async_trait::async_trait;
use serde::Serialize;

use domain::{GameError, GameEvent, GameId, PlayerId};

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
