use thiserror::Error;

use crate::PlayerId;

use super::config::GamePhase;

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
