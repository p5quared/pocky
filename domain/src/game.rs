use crate::PlayerId;
use rand::Rng;
use serde::Serialize;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum GameError {
    #[error("action {action} not valid in phase {phase:?}")]
    InvalidPhase { action: &'static str, phase: GamePhase },

    #[error("insufficient funds: have {available}, need {required}")]
    InsufficientFunds { available: i32, required: i32 },

    #[error("insufficient shares: have {available}, need {required}")]
    InsufficientShares { available: usize, required: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub enum GamePhase {
    Pending,
    Running,
    Ended,
}

#[derive(Clone)]
pub struct GameConfig {
    pub tick_interval_ms: u64,
    pub game_duration: u64,
    pub max_price_delta: i32,
    pub starting_price: i32,
    pub countdown_duration_ms: u64,
    pub starting_balance: i32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            tick_interval_ms: 500,
            game_duration: 360, // 360 ticks = 3 minutes
            max_price_delta: 25,
            starting_price: 100,
            countdown_duration_ms: 3000,
            starting_balance: 1000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerState {
    cash: i32,
    shares: Vec<i32>,
    pending_bids: Vec<i32>,
    pending_asks: Vec<i32>,
}

impl PlayerState {
    fn new(starting_cash: i32) -> Self {
        Self {
            cash: starting_cash,
            shares: Vec::new(),
            pending_bids: Vec::new(),
            pending_asks: Vec::new(),
        }
    }

    fn available_cash(&self) -> i32 {
        self.cash - self.pending_bids.iter().sum::<i32>()
    }

    fn available_shares(&self) -> usize {
        self.shares.len().saturating_sub(self.pending_asks.len())
    }

    #[allow(dead_code)]
    fn net_worth(
        &self,
        current_price: i32,
    ) -> i32 {
        self.cash + (self.shares.len() as i32 * current_price)
    }
}

#[derive(Clone)]
pub struct GameState {
    phase: GamePhase,
    config: GameConfig,
    current_price: i32,
    players: HashMap<PlayerId, PlayerState>,
}

#[derive(Clone, Copy)]
pub enum GameAction {
    Countdown(u32),
    Start,
    Tick,
    Bid { player_id: PlayerId, bid_value: i32 },
    Ask { player_id: PlayerId, ask_value: i32 },
    End,
}

#[derive(Clone, Copy, Serialize)]
pub enum GameEvent {
    Countdown(u32),
    GameStarted { starting_price: i32 },
    PriceChanged(i32),
    BidPlaced { player_id: PlayerId, bid_value: i32 },
    AskPlaced { player_id: PlayerId, ask_value: i32 },
    BidFilled { player_id: PlayerId, bid_value: i32 },
    AskFilled { player_id: PlayerId, ask_value: i32 },
    GameEnded,
}

#[derive(Clone, Copy)]
pub enum GameEffect {
    Notify { player_id: PlayerId, event: GameEvent },
    DelayedAction { delay_ms: u64, action: GameAction },
}

impl GameState {
    pub fn process_action(
        &mut self,
        action: GameAction,
    ) -> Result<Vec<GameEffect>, GameError> {
        match action {
            GameAction::Countdown(remaining) => self.handle_countdown(remaining),
            GameAction::Start => self.handle_start(),
            GameAction::Tick => self.handle_price_tick(),
            GameAction::Bid { player_id, bid_value } => self.handle_bid(player_id, bid_value),
            GameAction::Ask { player_id, ask_value } => self.handle_ask(player_id, ask_value),
            GameAction::End => self.handle_game_end(),
        }
    }
}

impl GameState {
    #[must_use]
    pub fn new(
        players: Vec<PlayerId>,
        config: GameConfig,
    ) -> Self {
        let starting_balance = config.starting_balance;
        let players = players
            .into_iter()
            .map(|pid| (pid, PlayerState::new(starting_balance)))
            .collect();
        Self {
            phase: GamePhase::Pending,
            config,
            players,
            current_price: 0,
        }
    }

    #[must_use]
    pub fn launch(
        players: Vec<PlayerId>,
        config: GameConfig,
    ) -> (Self, Vec<GameEffect>) {
        let state = Self::new(players.clone(), config.clone());

        let countdown_seconds = (config.countdown_duration_ms / 1000) as u32;

        let countdown_effects = (1..=countdown_seconds).rev().map(|remaining| {
            let delay_ms = u64::from(countdown_seconds - remaining) * 1000;
            GameEffect::DelayedAction {
                delay_ms,
                action: GameAction::Countdown(remaining),
            }
        });

        let start_effect = GameEffect::DelayedAction {
            delay_ms: config.countdown_duration_ms,
            action: GameAction::Start,
        };

        let effects = countdown_effects.chain(std::iter::once(start_effect)).collect();

        (state, effects)
    }

    fn handle_countdown(
        &self,
        remaining: u32,
    ) -> Result<Vec<GameEffect>, GameError> {
        Ok(self
            .players
            .keys()
            .map(|&player_id| GameEffect::Notify {
                player_id,
                event: GameEvent::Countdown(remaining),
            })
            .collect())
    }

    fn handle_start(&mut self) -> Result<Vec<GameEffect>, GameError> {
        if self.phase != GamePhase::Pending {
            return Err(GameError::InvalidPhase {
                action: "Start",
                phase: self.phase.clone(),
            });
        }

        self.phase = GamePhase::Running;
        self.current_price = self.config.starting_price;

        let started_notifications = self.players.keys().map(|&player_id| GameEffect::Notify {
            player_id,
            event: GameEvent::GameStarted {
                starting_price: self.current_price,
            },
        });

        let timed_effects = (1..self.config.game_duration).map(|tick| GameEffect::DelayedAction {
            delay_ms: tick * self.config.tick_interval_ms,
            action: if tick == self.config.game_duration - 1 {
                GameAction::End
            } else {
                GameAction::Tick
            },
        });

        Ok(started_notifications.chain(timed_effects).collect())
    }

    fn handle_price_tick(&mut self) -> Result<Vec<GameEffect>, GameError> {
        if self.phase != GamePhase::Running {
            return Err(GameError::InvalidPhase {
                action: "PriceTick",
                phase: self.phase.clone(),
            });
        }

        let mut rng = rand::thread_rng();
        let delta = rng.gen_range(-self.config.max_price_delta..=self.config.max_price_delta);
        self.current_price = (self.current_price + delta).max(0);

        let resolved_bids = self.resolve_bids();
        let resolved_asks = self.resolve_asks();

        let price_notifications = self.players.keys().map(|&player_id| GameEffect::Notify {
            player_id,
            event: GameEvent::PriceChanged(self.current_price),
        });

        let bid_notifications = resolved_bids.into_iter().map(|(player_id, bid_value)| GameEffect::Notify {
            player_id,
            event: GameEvent::BidFilled { player_id, bid_value },
        });

        let ask_notifications = resolved_asks.into_iter().map(|(player_id, ask_value)| GameEffect::Notify {
            player_id,
            event: GameEvent::AskFilled { player_id, ask_value },
        });

        let effects: Vec<GameEffect> = price_notifications
            .chain(bid_notifications)
            .chain(ask_notifications)
            .collect();

        Ok(effects)
    }

    fn handle_game_end(&mut self) -> Result<Vec<GameEffect>, GameError> {
        if self.phase != GamePhase::Running {
            return Err(GameError::InvalidPhase {
                action: "End",
                phase: self.phase.clone(),
            });
        }
        self.phase = GamePhase::Ended;

        Ok(self
            .players
            .keys()
            .map(|&player_id| GameEffect::Notify {
                player_id,
                event: GameEvent::GameEnded,
            })
            .collect())
    }

    fn resolve_bids(&mut self) -> Vec<(PlayerId, i32)> {
        let current_price = self.current_price;
        let mut resolved = Vec::new();

        for (player_id, state) in &mut self.players {
            let filled_indices: Vec<usize> = state
                .pending_bids
                .iter()
                .enumerate()
                .filter(|(_, bid)| **bid >= current_price)
                .map(|(i, _)| i)
                .collect();

            for i in filled_indices.into_iter().rev() {
                let bid_value = state.pending_bids.remove(i);
                state.shares.push(current_price);
                state.cash -= current_price;
                resolved.push((*player_id, bid_value));
            }
        }

        resolved
    }

    fn handle_bid(
        &mut self,
        player_id: PlayerId,
        bid_value: i32,
    ) -> Result<Vec<GameEffect>, GameError> {
        if self.phase != GamePhase::Running {
            return Err(GameError::InvalidPhase {
                action: "Bid",
                phase: self.phase.clone(),
            });
        }

        let state = self.players.get(&player_id);
        let available_player_balance = state.map(|s| s.available_cash()).unwrap_or(0);

        if bid_value > available_player_balance {
            return Err(GameError::InsufficientFunds {
                available: available_player_balance,
                required: bid_value,
            });
        }

        if let Some(state) = self.players.get_mut(&player_id) {
            state.pending_bids.push(bid_value);
        }

        Ok(self
            .players
            .keys()
            .map(|&pid| GameEffect::Notify {
                player_id: pid,
                event: GameEvent::BidPlaced { player_id, bid_value },
            })
            .collect())
    }

    fn handle_ask(
        &mut self,
        player_id: PlayerId,
        ask_value: i32,
    ) -> Result<Vec<GameEffect>, GameError> {
        if self.phase != GamePhase::Running {
            return Err(GameError::InvalidPhase {
                action: "Ask",
                phase: self.phase.clone(),
            });
        }

        let state = self.players.get(&player_id);
        let player_shares_available = state.map(|s| s.available_shares()).unwrap_or(0);

        if player_shares_available == 0 {
            return Err(GameError::InsufficientShares {
                available: player_shares_available,
                required: 1,
            });
        }

        if let Some(state) = self.players.get_mut(&player_id) {
            state.pending_asks.push(ask_value);
        }

        Ok(self
            .players
            .keys()
            .map(|&pid| GameEffect::Notify {
                player_id: pid,
                event: GameEvent::AskPlaced { player_id, ask_value },
            })
            .collect())
    }

    fn resolve_asks(&mut self) -> Vec<(PlayerId, i32)> {
        let current_price = self.current_price;
        let mut resolved = Vec::new();

        for (player_id, state) in &mut self.players {
            let filled_indices: Vec<usize> = state
                .pending_asks
                .iter()
                .enumerate()
                .filter(|(_, ask)| **ask <= current_price)
                .map(|(i, _)| i)
                .collect();

            for i in filled_indices.into_iter().rev() {
                let ask_value = state.pending_asks.remove(i);
                if !state.shares.is_empty() {
                    state.shares.pop();
                }
                state.cash += current_price;
                resolved.push((*player_id, ask_value));
            }
        }

        resolved
    }

    #[cfg(test)]
    fn get_player(
        &self,
        player_id: PlayerId,
    ) -> Option<&PlayerState> {
        self.players.get(&player_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> GameConfig {
        GameConfig {
            tick_interval_ms: 1000,
            game_duration: 10,
            max_price_delta: 10,
            starting_price: 50,
            countdown_duration_ms: 3000,
            starting_balance: 100,
        }
    }

    /// Create a game already in Running state at the given price
    fn create_running_game(
        players: Vec<PlayerId>,
        price: i32,
    ) -> GameState {
        let mut config = test_config();
        config.starting_price = price;
        let mut game = GameState::new(players, config);
        game.process_action(GameAction::Start).unwrap();
        game
    }

    fn assert_cash(
        state: &GameState,
        player_id: PlayerId,
        want_balance: i32,
    ) {
        let player = state.get_player(player_id).expect("player not found");
        let got_balance = player.available_cash();

        assert_eq!(
            got_balance, want_balance,
            "Expected cash balance for player {:?} to be {}, but got {}",
            player_id, want_balance, got_balance
        );
    }

    fn assert_shares(
        state: &GameState,
        player_id: PlayerId,
        want_count: usize,
        want_total: i32,
    ) {
        let player = state.get_player(player_id).expect("player not found");
        let got_balance: i32 = player.shares.iter().sum();
        assert_eq!(
            got_balance, want_total,
            "Expected total share value for player {:?} to be {}, but got {}",
            player_id, want_total, got_balance
        );

        let got_count = player.shares.len();

        assert_eq!(
            got_count, want_count,
            "Expected {} shares for player {:?}, but got {}",
            want_count, player_id, got_count
        );
    }

    fn assert_open_bids(
        state: &GameState,
        player_id: PlayerId,
        want_num_bids: usize,
        want_total_value: i32,
    ) {
        let player = state.get_player(player_id).expect("player not found");
        let got_bid_count = player.pending_bids.len();
        let got_total_value: i32 = player.pending_bids.iter().sum();

        assert_eq!(
            want_num_bids, got_bid_count,
            "Expected {} open bids for player {:?}, but got {}",
            want_num_bids, player_id, got_bid_count
        );

        assert_eq!(
            want_total_value, got_total_value,
            "Expected total bid value for player {:?} to be {}, but got {}",
            player_id, want_total_value, got_total_value,
        );
    }

    fn assert_open_asks(
        state: &GameState,
        player_id: PlayerId,
        want_num_asks: usize,
        want_total_value: i32,
    ) {
        let player = state.get_player(player_id).expect("player not found");
        let got_ask_count = player.pending_asks.len();
        let got_total_value: i32 = player.pending_asks.iter().sum();
        assert_eq!(
            want_num_asks, got_ask_count,
            "Expected {} open asks for player {:?}, but got {}",
            want_num_asks, player_id, got_ask_count
        );
        assert_eq!(
            want_total_value, got_total_value,
            "Expected total ask value for player {:?} to be {}, but got {}",
            player_id, want_total_value, got_total_value,
        );
    }

    #[test]
    fn test_transactions() {
        let p = PlayerId(uuid::Uuid::new_v4());
        // Start at price 0 so bids don't immediately resolve
        let mut engine = create_running_game(vec![p], 0);
        engine
            .process_action(GameAction::Bid {
                player_id: p,
                bid_value: 20,
            })
            .unwrap();
        engine
            .process_action(GameAction::Bid {
                player_id: p,
                bid_value: 40,
            })
            .unwrap();
        engine
            .process_action(GameAction::Bid {
                player_id: p,
                bid_value: 40,
            })
            .unwrap();

        assert_cash(&engine, p, 0);
        assert_open_bids(&engine, p, 3, 100);

        // Set price directly and resolve (simulating what PriceTick does)
        engine.current_price = 30;
        engine.resolve_bids();
        // 2 bids for 40 filled @30, refund 10 each
        assert_cash(&engine, p, 20);
        assert_shares(&engine, p, 2, 60);
        assert_open_bids(&engine, p, 1, 20);

        engine
            .process_action(GameAction::Ask {
                player_id: p,
                ask_value: 75,
            })
            .unwrap();
        assert_open_asks(&engine, p, 1, 75);

        // Set price directly and resolve
        engine.current_price = 100;
        engine.resolve_asks();
        // ask filled @100
        assert_cash(&engine, p, 120);
        assert_shares(&engine, p, 1, 30);
        assert_open_asks(&engine, p, 0, 0);
    }

    #[test]
    fn test_bid_insufficient_funds() {
        let valid_player = PlayerId(uuid::Uuid::new_v4());
        let invalid_player = PlayerId(uuid::Uuid::new_v4());
        let mut engine = create_running_game(vec![valid_player], 50);

        // Player not in game has 0 balance
        let result = engine.process_action(GameAction::Bid {
            player_id: invalid_player,
            bid_value: 50,
        });

        assert!(matches!(
            result,
            Err(GameError::InsufficientFunds {
                available: 0,
                required: 50
            })
        ));
    }

    #[test]
    fn test_ask_insufficient_shares() {
        let p = PlayerId(uuid::Uuid::new_v4());
        let mut engine = create_running_game(vec![p], 50);

        // No shares owned, ask should return error
        let result = engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 50,
        });

        assert!(matches!(
            result,
            Err(GameError::InsufficientShares {
                available: 0,
                required: 1
            })
        ));
    }

    #[test]
    fn test_start_game() {
        let p1 = PlayerId(uuid::Uuid::new_v4());
        let p2 = PlayerId(uuid::Uuid::new_v4());
        let mut config = test_config();
        config.starting_price = 50;
        let mut engine = GameState::new(vec![p1, p2], config);

        assert_eq!(engine.phase, GamePhase::Pending);

        let effects = engine.process_action(GameAction::Start).unwrap();

        assert_eq!(engine.phase, GamePhase::Running);
        assert_eq!(engine.current_price, 50);

        // Should have GameStarted notifications for both players + SchedulePriceTick
        let started_notifications: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    GameEffect::Notify {
                        event: GameEvent::GameStarted { .. },
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(started_notifications.len(), 2);

        assert!(effects.iter().any(|e| matches!(
            e,
            GameEffect::DelayedAction {
                delay_ms: 1000,
                action: GameAction::Tick
            }
        )),);
    }

    #[test]
    fn test_price_tick() {
        let p1 = PlayerId(uuid::Uuid::new_v4());
        let mut engine = create_running_game(vec![p1], 50);

        let effects = engine.process_action(GameAction::Tick).unwrap();

        // Price should have changed (within delta range)
        let price_delta = (engine.current_price - 50).abs();
        assert!(price_delta <= 10, "Price delta {} exceeds max_delta 10", price_delta);

        // Should have PriceChanged notification + SchedulePriceTick
        assert!(effects.iter().any(|e| matches!(
            e,
            GameEffect::Notify {
                event: GameEvent::PriceChanged(_),
                ..
            }
        )));
    }

    #[test]
    fn test_bid_resolved_notifications() {
        let p = PlayerId(uuid::Uuid::new_v4());
        // Start at price 0 so the bid doesn't immediately resolve
        let mut engine = create_running_game(vec![p], 0);

        engine
            .process_action(GameAction::Bid {
                player_id: p,
                bid_value: 40,
            })
            .unwrap();

        // Set price to 30 and process a tick to trigger resolution
        engine.current_price = 30;
        let resolved = engine.resolve_bids();

        // Should have resolved the bid
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0], (p, 40));
    }

    #[test]
    fn test_ask_resolved_notifications() {
        let p = PlayerId(uuid::Uuid::new_v4());
        // Start at price 50 so bid resolves immediately
        let mut engine = create_running_game(vec![p], 50);

        // Buy a share first
        engine
            .process_action(GameAction::Bid {
                player_id: p,
                bid_value: 50,
            })
            .unwrap();
        // Resolve the bid at current price
        engine.resolve_bids();
        assert_shares(&engine, p, 1, 50);

        // Place an ask
        engine
            .process_action(GameAction::Ask {
                player_id: p,
                ask_value: 60,
            })
            .unwrap();

        // Price goes up, ask should be resolved
        engine.current_price = 70;
        let resolved = engine.resolve_asks();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0], (p, 60));
        assert_shares(&engine, p, 0, 0);
    }

    #[test]
    fn test_bid_placed_notifications() {
        let p1 = PlayerId(uuid::Uuid::new_v4());
        let p2 = PlayerId(uuid::Uuid::new_v4());
        let mut engine = create_running_game(vec![p1, p2], 50);

        let effects = engine
            .process_action(GameAction::Bid {
                player_id: p1,
                bid_value: 50,
            })
            .unwrap();

        // Both players should be notified of the bid
        assert_eq!(effects.len(), 2);
        let notified_players: Vec<_> = effects
            .iter()
            .filter_map(|e| match e {
                GameEffect::Notify {
                    player_id,
                    event:
                        GameEvent::BidPlaced {
                            player_id: bidder,
                            bid_value: 50,
                        },
                } if *bidder == p1 => Some(*player_id),
                _ => None,
            })
            .collect();
        assert!(notified_players.contains(&p1));
        assert!(notified_players.contains(&p2));
    }

    #[test]
    fn test_ask_placed_notifications() {
        let p1 = PlayerId(uuid::Uuid::new_v4());
        let p2 = PlayerId(uuid::Uuid::new_v4());
        // Start at price 50 so bid resolves immediately
        let mut engine = create_running_game(vec![p1, p2], 50);

        // p1 needs to own a share first
        engine
            .process_action(GameAction::Bid {
                player_id: p1,
                bid_value: 50,
            })
            .unwrap();
        engine.resolve_bids();

        let effects = engine
            .process_action(GameAction::Ask {
                player_id: p1,
                ask_value: 60,
            })
            .unwrap();

        // Both players should be notified of the ask
        assert_eq!(effects.len(), 2);
        let notified_players: Vec<_> = effects
            .iter()
            .filter_map(|e| match e {
                GameEffect::Notify {
                    player_id,
                    event:
                        GameEvent::AskPlaced {
                            player_id: asker,
                            ask_value: 60,
                        },
                } if *asker == p1 => Some(*player_id),
                _ => None,
            })
            .collect();
        assert!(notified_players.contains(&p1));
        assert!(notified_players.contains(&p2));
    }

    #[test]
    fn test_game_end_notifications() {
        let p1 = PlayerId(uuid::Uuid::new_v4());
        let p2 = PlayerId(uuid::Uuid::new_v4());
        let mut engine = create_running_game(vec![p1, p2], 50);

        let effects = engine.process_action(GameAction::End).unwrap();

        // Should notify both players of game end
        assert_eq!(effects.len(), 2);
        let notified_players: Vec<_> = effects
            .iter()
            .filter_map(|e| match e {
                GameEffect::Notify {
                    player_id,
                    event: GameEvent::GameEnded,
                } => Some(*player_id),
                _ => None,
            })
            .collect();
        assert!(notified_players.contains(&p1));
        assert!(notified_players.contains(&p2));
    }

    #[test]
    fn test_ask_error_when_insufficient_shares() {
        let p = PlayerId(uuid::Uuid::new_v4());
        // Start at price 50 so bid resolves immediately
        let mut engine = create_running_game(vec![p], 50);

        // Buy one share
        engine
            .process_action(GameAction::Bid {
                player_id: p,
                bid_value: 50,
            })
            .unwrap();
        engine.resolve_bids();
        assert_shares(&engine, p, 1, 50);

        // First ask should succeed
        let effects = engine
            .process_action(GameAction::Ask {
                player_id: p,
                ask_value: 60,
            })
            .unwrap();
        assert!(
            effects.iter().any(|e| matches!(
                e,
                GameEffect::Notify {
                    event: GameEvent::AskPlaced { .. },
                    ..
                }
            )),
            "First ask should be placed"
        );
        assert_open_asks(&engine, p, 1, 60);

        // Second ask should return InsufficientShares error - only 1 share but already 1 open ask
        let result = engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 70,
        });
        assert!(matches!(
            result,
            Err(GameError::InsufficientShares {
                available: 0,
                required: 1
            })
        ));
        assert_open_asks(&engine, p, 1, 60);
    }

    #[test]
    fn test_bid_error_when_not_running() {
        let p = PlayerId(uuid::Uuid::new_v4());
        let mut engine = GameState::new(vec![p], test_config());

        // Game is in Pending state, bid should return InvalidPhase error
        let result = engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 50,
        });

        assert!(matches!(
            result,
            Err(GameError::InvalidPhase {
                action: "Bid",
                phase: GamePhase::Pending
            })
        ));
    }

    #[test]
    fn test_price_stays_non_negative() {
        let p = PlayerId(uuid::Uuid::new_v4());
        // Start at price 0
        let mut engine = create_running_game(vec![p], 0);

        // Run many ticks to test that price never goes negative
        for _ in 0..100 {
            engine.process_action(GameAction::Tick).unwrap();
            assert!(engine.current_price >= 0, "Price should never be negative");
        }
    }
}
