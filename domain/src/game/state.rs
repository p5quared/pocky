use std::collections::HashMap;
use std::time::Duration;

use crate::PlayerId;

use super::ticker::PlayerTicker;
use super::{GameAction, GameConfig, GameEffect, GameError, GameEvent, GamePhase};

#[derive(Clone, Debug)]
pub(super) struct PlayerState {
    pub(super) cash: i32,
    pub(super) shares: Vec<i32>,
    pub(super) open_bids: Vec<i32>,
    pub(super) open_asks: Vec<i32>,
}

impl PlayerState {
    pub(super) fn new(starting_cash: i32) -> Self {
        Self {
            cash: starting_cash,
            shares: Vec::new(),
            open_bids: Vec::new(),
            open_asks: Vec::new(),
        }
    }

    pub(super) fn available_cash(&self) -> i32 {
        self.cash - self.open_bids.iter().sum::<i32>()
    }

    pub(super) fn available_shares(&self) -> usize {
        self.shares.len().saturating_sub(self.open_asks.len())
    }

    pub(super) fn net_worth(
        &self,
        current_price: i32,
    ) -> i32 {
        self.cash + (self.shares.len() as i32 * current_price)
    }
}

#[derive(Clone)]
pub struct GameState {
    pub(super) phase: GamePhase,
    config: GameConfig,
    pub(super) players: HashMap<PlayerId, PlayerState>,
    pub(super) player_tickers: HashMap<PlayerId, PlayerTicker>,
    pub(super) ticks_remaining: u32,
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
            GameAction::CancelBid { player_id, price } => self.handle_cancel_bid(player_id, price),
            GameAction::CancelAsk { player_id, price } => self.handle_cancel_ask(player_id, price),
            GameAction::End => self.handle_game_end(),
        }
    }

    fn require_phase(
        &self,
        required: GamePhase,
        action: &'static str,
    ) -> Result<(), GameError> {
        if self.phase != required {
            return Err(GameError::InvalidPhase {
                action,
                phase: self.phase.clone(),
            });
        }
        Ok(())
    }
}

impl GameState {
    #[must_use]
    pub fn new(
        players: Vec<PlayerId>,
        config: GameConfig,
    ) -> Self {
        let starting_balance = config.starting_balance;
        let tick_count = (config.game_duration.as_millis() / config.tick_interval.as_millis()) as u32;
        let player_ids: Vec<PlayerId> = players.clone();
        let players = players
            .into_iter()
            .map(|pid| (pid, PlayerState::new(starting_balance)))
            .collect();
        let player_tickers = player_ids
            .into_iter()
            .map(|pid| (pid, PlayerTicker::new(config.max_price_delta, 0)))
            .collect();
        Self {
            phase: GamePhase::Pending,
            config,
            players,
            player_tickers,
            ticks_remaining: tick_count,
        }
    }

    #[must_use]
    pub fn launch(
        players: Vec<PlayerId>,
        config: GameConfig,
    ) -> (Self, Vec<GameEffect>) {
        let state = Self::new(players.clone(), config.clone());

        let countdown_seconds = config.countdown_duration.as_secs() as u32;

        let countdown_effects = (1..=countdown_seconds).rev().map(move |remaining| {
            let delay = Duration::from_secs(u64::from(countdown_seconds - remaining));
            GameEffect::DelayedAction {
                delay,
                action: GameAction::Countdown(remaining),
            }
        });

        let start_effect = GameEffect::DelayedAction {
            delay: config.countdown_duration,
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
            .map(|&player_id| GameEffect::Notification {
                player_id,
                event: GameEvent::Countdown(remaining),
            })
            .collect())
    }

    fn handle_start(&mut self) -> Result<Vec<GameEffect>, GameError> {
        self.require_phase(GamePhase::Pending, "Start")?;

        self.phase = GamePhase::Running;

        // Initialize all player tickers to starting price
        let starting_price = self.config.starting_price;
        for player_ticker in self.player_tickers.values_mut() {
            player_ticker.current_price = starting_price;
        }

        let player_ids: Vec<PlayerId> = self.players.keys().copied().collect();

        let started_notifications = player_ids.iter().map(|&player_id| GameEffect::Notification {
            player_id,
            event: GameEvent::GameStarted {
                starting_price,
                starting_balance: self.config.starting_balance,
                players: player_ids.clone(),
                game_duration_secs: self.config.game_duration.as_secs(),
            },
        });

        let first_tick_effect = GameEffect::DelayedAction {
            delay: self.config.tick_interval,
            action: GameAction::Tick,
        };

        Ok(started_notifications.chain(std::iter::once(first_tick_effect)).collect())
    }

    fn handle_price_tick(&mut self) -> Result<Vec<GameEffect>, GameError> {
        self.require_phase(GamePhase::Running, "PriceTick")?;

        if self.ticks_remaining == 0 {
            return Err(GameError::InvalidPhase {
                action: "PriceTick",
                phase: GamePhase::Ended,
            });
        }

        self.ticks_remaining -= 1;

        for player_ticker in self.player_tickers.values_mut() {
            player_ticker.tick();
        }

        let resolved_bids = self.resolve_bids();
        let resolved_asks = self.resolve_asks();

        let player_ids: Vec<PlayerId> = self.players.keys().copied().collect();
        let prices: Vec<(PlayerId, i32)> = self.player_tickers.iter().map(|(&pid, pt)| (pid, pt.current_price)).collect();

        let price_notifications = player_ids.iter().flat_map(|&notify_player| {
            prices.iter().map(move |&(ticker_owner, price)| GameEffect::Notification {
                player_id: notify_player,
                event: GameEvent::PriceChanged {
                    player_id: ticker_owner,
                    price,
                },
            })
        });

        let bid_notifications = resolved_bids.into_iter().flat_map(|(order_owner, bid_value)| {
            player_ids.iter().map(move |&notify_player| GameEffect::Notification {
                player_id: notify_player,
                event: GameEvent::BidFilled {
                    player_id: order_owner,
                    bid_value,
                },
            })
        });

        let ask_notifications = resolved_asks.into_iter().flat_map(|(order_owner, ask_value)| {
            player_ids.iter().map(move |&notify_player| GameEffect::Notification {
                player_id: notify_player,
                event: GameEvent::AskFilled {
                    player_id: order_owner,
                    ask_value,
                },
            })
        });

        let next_action = if self.ticks_remaining == 0 {
            GameAction::End
        } else {
            GameAction::Tick
        };

        let next_tick_effect = GameEffect::DelayedAction {
            delay: self.config.tick_interval,
            action: next_action,
        };

        let effects: Vec<GameEffect> = price_notifications
            .chain(bid_notifications)
            .chain(ask_notifications)
            .chain(std::iter::once(next_tick_effect))
            .collect();

        Ok(effects)
    }

    fn handle_game_end(&mut self) -> Result<Vec<GameEffect>, GameError> {
        self.require_phase(GamePhase::Running, "End")?;
        self.phase = GamePhase::Ended;

        let final_balances: Vec<(PlayerId, i32)> = self
            .players
            .iter()
            .map(|(&player_id, state)| {
                let player_price = self.player_tickers.get(&player_id).map(|pt| pt.current_price).unwrap_or(0);
                (player_id, state.net_worth(player_price))
            })
            .collect();

        Ok(self
            .players
            .keys()
            .map(|&player_id| GameEffect::Notification {
                player_id,
                event: GameEvent::GameEnded {
                    final_balances: final_balances.clone(),
                },
            })
            .collect())
    }

    pub(super) fn resolve_bids(&mut self) -> Vec<(PlayerId, i32)> {
        let player_prices: HashMap<PlayerId, i32> =
            self.player_tickers.iter().map(|(&pid, pt)| (pid, pt.current_price)).collect();

        let mut resolved = Vec::new();
        for (player_id, state) in &mut self.players {
            let current_price = player_prices.get(player_id).copied().unwrap_or(0);
            let can_fill_bid = |bid: i32| bid >= current_price;

            let filled_indices: Vec<usize> = state
                .open_bids
                .iter()
                .enumerate()
                .filter(|(_, bid)| can_fill_bid(**bid))
                .map(|(i, _)| i)
                .collect();

            for i in filled_indices.into_iter().rev() {
                let bid_value = state.open_bids.remove(i);
                state.shares.push(current_price);
                state.cash -= current_price;
                resolved.push((*player_id, bid_value));
            }
        }

        for &(player_id, _) in &resolved {
            let fill_price = player_prices.get(&player_id).copied().unwrap_or(0);
            for player_ticker in self.player_tickers.values_mut() {
                player_ticker.ticker.on_bid_filled(fill_price as f32);
            }
        }

        resolved
    }

    fn handle_bid(
        &mut self,
        player_id: PlayerId,
        bid_value: i32,
    ) -> Result<Vec<GameEffect>, GameError> {
        self.require_phase(GamePhase::Running, "Bid")?;

        let state = self.players.get(&player_id);
        let available_player_balance = state.map(|s| s.available_cash()).unwrap_or(0);

        if bid_value > available_player_balance {
            return Err(GameError::InsufficientFunds {
                available: available_player_balance,
                required: bid_value,
            });
        }

        if let Some(state) = self.players.get_mut(&player_id) {
            state.open_bids.push(bid_value);
        }

        for player_ticker in self.player_tickers.values_mut() {
            player_ticker.ticker.on_bid_placed(bid_value as f32);
        }

        Ok(self
            .players
            .keys()
            .map(|&pid| GameEffect::Notification {
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
        self.require_phase(GamePhase::Running, "Ask")?;

        let state = self.players.get(&player_id);
        let player_shares_available = state.map(|s| s.available_shares()).unwrap_or(0);

        if player_shares_available == 0 {
            return Err(GameError::InsufficientShares {
                available: player_shares_available,
                required: 1,
            });
        }

        if let Some(state) = self.players.get_mut(&player_id) {
            state.open_asks.push(ask_value);
        }

        for player_ticker in self.player_tickers.values_mut() {
            player_ticker.ticker.on_ask_placed(ask_value as f32);
        }

        Ok(self
            .players
            .keys()
            .map(|&pid| GameEffect::Notification {
                player_id: pid,
                event: GameEvent::AskPlaced { player_id, ask_value },
            })
            .collect())
    }

    fn handle_cancel_bid(
        &mut self,
        player_id: PlayerId,
        price: i32,
    ) -> Result<Vec<GameEffect>, GameError> {
        self.require_phase(GamePhase::Running, "CancelBid")?;

        let state = self.players.get_mut(&player_id).ok_or(GameError::PlayerNotFound(player_id))?;

        let idx = state
            .open_bids
            .iter()
            .position(|&b| b == price)
            .ok_or(GameError::OrderNotFound {
                order_type: "bid".to_string(),
                price,
            })?;

        state.open_bids.remove(idx);

        Ok(self
            .players
            .keys()
            .map(|&pid| GameEffect::Notification {
                player_id: pid,
                event: GameEvent::BidCanceled { player_id, price },
            })
            .collect())
    }

    fn handle_cancel_ask(
        &mut self,
        player_id: PlayerId,
        price: i32,
    ) -> Result<Vec<GameEffect>, GameError> {
        self.require_phase(GamePhase::Running, "CancelAsk")?;

        let state = self.players.get_mut(&player_id).ok_or(GameError::PlayerNotFound(player_id))?;

        let idx = state
            .open_asks
            .iter()
            .position(|&a| a == price)
            .ok_or(GameError::OrderNotFound {
                order_type: "ask".to_string(),
                price,
            })?;

        state.open_asks.remove(idx);

        Ok(self
            .players
            .keys()
            .map(|&pid| GameEffect::Notification {
                player_id: pid,
                event: GameEvent::AskCanceled { player_id, price },
            })
            .collect())
    }

    pub(super) fn resolve_asks(&mut self) -> Vec<(PlayerId, i32)> {
        let player_prices: HashMap<PlayerId, i32> =
            self.player_tickers.iter().map(|(&pid, pt)| (pid, pt.current_price)).collect();

        let mut resolved = Vec::new();

        for (player_id, state) in &mut self.players {
            let current_price = player_prices.get(player_id).copied().unwrap_or(0);
            let can_resolve_ask = |ask: i32| ask <= current_price;

            let filled_indices: Vec<usize> = state
                .open_asks
                .iter()
                .enumerate()
                .filter(|(_, ask)| can_resolve_ask(**ask))
                .map(|(i, _)| i)
                .collect();

            for i in filled_indices.into_iter().rev() {
                let ask_value = state.open_asks.remove(i);
                if !state.shares.is_empty() {
                    state.shares.pop();
                }
                state.cash += current_price;
                resolved.push((*player_id, ask_value));
            }
        }

        for &(player_id, _) in &resolved {
            let fill_price = player_prices.get(&player_id).copied().unwrap_or(0);
            for player_ticker in self.player_tickers.values_mut() {
                player_ticker.ticker.on_ask_filled(fill_price as f32);
            }
        }

        resolved
    }

    #[cfg(test)]
    pub(super) fn get_player(
        &self,
        player_id: PlayerId,
    ) -> Option<&PlayerState> {
        self.players.get(&player_id)
    }
}
