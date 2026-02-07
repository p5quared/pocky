mod state;
mod ticker;

#[cfg(test)]
mod tests;

pub use state::GameState;
pub use ticker::{Decay, MarketConditions, MarketForce, PlayerTicker, Ticker};

use std::time::Duration;

use crate::PlayerId;

#[derive(Clone, Copy, Debug)]
pub enum GameAction {
    Countdown(u32),
    Start,
    Tick,
    Bid { player_id: PlayerId, bid_value: i32 },
    Ask { player_id: PlayerId, ask_value: i32 },
    CancelBid { player_id: PlayerId, price: i32 },
    CancelAsk { player_id: PlayerId, price: i32 },
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GamePhase {
    Pending,
    Running,
    Ended,
}

#[derive(Clone)]
pub struct GameConfig {
    pub tick_interval: Duration,
    pub game_duration: Duration,
    pub max_price_delta: i32,
    pub starting_price: i32,
    pub countdown_duration: Duration,
    pub starting_balance: i32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(250),
            game_duration: Duration::from_secs(180),
            max_price_delta: 25,
            starting_price: 100,
            countdown_duration: Duration::from_secs(3),
            starting_balance: 1000,
        }
    }
}

#[derive(Clone, Debug)]
pub enum GameEffect {
    Notification { player_id: PlayerId, event: GameEvent },
    DelayedAction { delay: Duration, action: GameAction },
}

/// GameEvents are the visible messages that can be sent as notifications
/// via `crate::GameEffect`
#[derive(Clone, Debug)]
pub enum GameEvent {
    Countdown(u32),
    GameStarted {
        starting_price: i32,
        starting_balance: i32,
        players: Vec<PlayerId>,
        game_duration_secs: u64,
    },
    PriceChanged {
        player_id: PlayerId,
        price: i32,
    },
    BidPlaced {
        player_id: PlayerId,
        bid_value: i32,
    },
    AskPlaced {
        player_id: PlayerId,
        ask_value: i32,
    },
    BidFilled {
        player_id: PlayerId,
        bid_value: i32,
    },
    AskFilled {
        player_id: PlayerId,
        ask_value: i32,
    },
    BidCanceled {
        player_id: PlayerId,
        price: i32,
    },
    AskCanceled {
        player_id: PlayerId,
        price: i32,
    },
    GameEnded {
        final_balances: Vec<(PlayerId, i32)>,
    },
}

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum GameError {
    #[error("action {action} not valid in phase {phase:?}")]
    InvalidPhase { action: &'static str, phase: GamePhase },

    #[error("insufficient funds: have {available}, need {required}")]
    InsufficientFunds { available: i32, required: i32 },

    #[error("insufficient shares: have {available}, need {required}")]
    InsufficientShares { available: usize, required: usize },

    #[error("player not found: {0:?}")]
    PlayerNotFound(PlayerId),

    #[error("{order_type} order at price {price} not found")]
    OrderNotFound { order_type: String, price: i32 },
}
