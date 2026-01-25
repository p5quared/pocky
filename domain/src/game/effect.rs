use std::time::Duration;

use crate::PlayerId;

use super::{action::GameAction, event::GameEvent};

#[derive(Clone, Debug)]
pub enum GameEffect {
    Notify { player_id: PlayerId, event: GameEvent },
    DelayedAction { delay: Duration, action: GameAction },
}
