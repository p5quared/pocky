use async_trait::async_trait;
use serde::Serialize;

use domain::{GameError, GameId, PlayerId};

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
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameNotification {
    Countdown {
        game_id: GameId,
        remaining: u32,
    },
    GameStarted {
        game_id: GameId,
        starting_price: i32,
        starting_balance: i32,
        players: Vec<PlayerId>,
    },
    PriceChanged {
        game_id: GameId,
        price: i32,
    },
    BidPlaced {
        game_id: GameId,
        player_id: PlayerId,
        bid_value: i32,
    },
    AskPlaced {
        game_id: GameId,
        player_id: PlayerId,
        ask_value: i32,
    },
    BidFilled {
        game_id: GameId,
        player_id: PlayerId,
        bid_value: i32,
    },
    AskFilled {
        game_id: GameId,
        player_id: PlayerId,
        ask_value: i32,
    },
    GameEnded {
        game_id: GameId,
        final_balances: Vec<(PlayerId, i32)>,
    },
}

#[async_trait]
pub trait GameEventNotifier: Send + Sync {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: GameNotification,
    );
}
