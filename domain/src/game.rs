use std::collections::HashMap;
use std::time::Duration;

use rand::Rng;
use serde::Serialize;
use thiserror::Error;

use crate::PlayerId;

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
            tick_interval: Duration::from_millis(500),
            game_duration: Duration::from_secs(180),
            max_price_delta: 25,
            starting_price: 100,
            countdown_duration: Duration::from_secs(3),
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

#[derive(Clone, Copy, Debug)]
pub enum GameAction {
    Countdown(u32),
    Start,
    Tick,
    Bid { player_id: PlayerId, bid_value: i32 },
    Ask { player_id: PlayerId, ask_value: i32 },
    End,
}

#[derive(Clone, Debug, Serialize)]
pub enum GameEvent {
    Countdown(u32),
    GameStarted {
        starting_price: i32,
        starting_balance: i32,
        players: Vec<PlayerId>,
    },
    PriceChanged(i32),
    BidPlaced { player_id: PlayerId, bid_value: i32 },
    AskPlaced { player_id: PlayerId, ask_value: i32 },
    BidFilled { player_id: PlayerId, bid_value: i32 },
    AskFilled { player_id: PlayerId, ask_value: i32 },
    GameEnded,
}

#[derive(Clone, Debug)]
pub enum GameEffect {
    Notify { player_id: PlayerId, event: GameEvent },
    DelayedAction { delay: Duration, action: GameAction },
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

        let player_ids: Vec<PlayerId> = self.players.keys().copied().collect();

        let started_notifications = player_ids.iter().map(|&player_id| GameEffect::Notify {
            player_id,
            event: GameEvent::GameStarted {
                starting_price: self.current_price,
                starting_balance: self.config.starting_balance,
                players: player_ids.clone(),
            },
        });

        let tick_count = (self.config.game_duration.as_millis() / self.config.tick_interval.as_millis()) as u32;
        let tick_interval = self.config.tick_interval;

        let timed_effects = (1..tick_count).map(move |tick| GameEffect::DelayedAction {
            delay: tick_interval * tick,
            action: if tick == tick_count - 1 {
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
        let can_fill_bid = |bid: i32| bid >= current_price;

        let mut resolved = Vec::new();
        for (player_id, state) in &mut self.players {
            let filled_indices: Vec<usize> = state
                .pending_bids
                .iter()
                .enumerate()
                .filter(|(_, bid)| can_fill_bid(**bid))
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
        let can_resolve_ask = |ask: i32| ask <= current_price;
        let mut resolved = Vec::new();

        for (player_id, state) in &mut self.players {
            let filled_indices: Vec<usize> = state
                .pending_asks
                .iter()
                .enumerate()
                .filter(|(_, ask)| can_resolve_ask(**ask))
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

    #[derive(Default, Clone)]
    struct ExpectedPlayer {
        cash: Option<i32>,
        shares: Option<usize>,
        bids: Option<usize>,
        asks: Option<usize>,
    }

    fn player() -> ExpectedPlayer {
        ExpectedPlayer::default()
    }

    impl ExpectedPlayer {
        fn cash(
            mut self,
            cash: i32,
        ) -> Self {
            self.cash = Some(cash);
            self
        }

        fn shares(
            mut self,
            count: usize,
        ) -> Self {
            self.shares = Some(count);
            self
        }

        fn bids(
            mut self,
            count: usize,
        ) -> Self {
            self.bids = Some(count);
            self
        }

        fn asks(
            mut self,
            count: usize,
        ) -> Self {
            self.asks = Some(count);
            self
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum ExpectedOutcome {
        Ok,
        InsufficientFunds { available: i32, required: i32 },
        InsufficientShares { available: usize, required: usize },
        InvalidPhase { action: &'static str },
    }

    struct TestHarness {
        game: GameState,
        players: Vec<PlayerId>,
        last_result: Result<Vec<GameEffect>, GameError>,
    }

    impl TestHarness {
        fn new(num_players: usize) -> Self {
            let players: Vec<PlayerId> = (0..num_players).map(|_| PlayerId(uuid::Uuid::new_v4())).collect();
            let game = GameState::new(players.clone(), test_config());
            Self {
                game,
                players,
                last_result: Ok(vec![]),
            }
        }

        fn at_price(
            mut self,
            price: i32,
        ) -> Self {
            self.game.current_price = price;
            self.game.phase = GamePhase::Running;
            self
        }

        fn pending(self) -> Self {
            self
        }

        fn bid(
            &mut self,
            player_idx: usize,
            value: i32,
        ) -> &mut Self {
            let player_id = self.players[player_idx];
            self.last_result = self.game.process_action(GameAction::Bid {
                player_id,
                bid_value: value,
            });
            self
        }

        fn ask(
            &mut self,
            player_idx: usize,
            value: i32,
        ) -> &mut Self {
            let player_id = self.players[player_idx];
            self.last_result = self.game.process_action(GameAction::Ask {
                player_id,
                ask_value: value,
            });
            self
        }

        fn start(&mut self) -> &mut Self {
            self.last_result = self.game.process_action(GameAction::Start);
            self
        }

        fn tick(&mut self) -> &mut Self {
            self.last_result = self.game.process_action(GameAction::Tick);
            self
        }

        fn end(&mut self) -> &mut Self {
            self.last_result = self.game.process_action(GameAction::End);
            self
        }

        fn set_price(
            &mut self,
            price: i32,
        ) -> &mut Self {
            self.game.current_price = price;
            self
        }

        fn resolve_bids(&mut self) -> &mut Self {
            self.game.resolve_bids();
            self
        }

        fn resolve_asks(&mut self) -> &mut Self {
            self.game.resolve_asks();
            self
        }

        #[track_caller]
        fn check(
            &self,
            player_idx: usize,
            expected: ExpectedPlayer,
        ) -> &Self {
            let player_id = self.players[player_idx];
            let state = self.game.get_player(player_id).expect("player not found");

            if let Some(expected_cash) = expected.cash {
                let actual = state.available_cash();
                assert_eq!(
                    actual, expected_cash,
                    "Player {}: expected cash {}, got {}",
                    player_idx, expected_cash, actual
                );
            }

            if let Some(expected_shares) = expected.shares {
                let actual = state.shares.len();
                assert_eq!(
                    actual, expected_shares,
                    "Player {}: expected {} shares, got {}",
                    player_idx, expected_shares, actual
                );
            }

            if let Some(expected_bids) = expected.bids {
                let actual = state.pending_bids.len();
                assert_eq!(
                    actual, expected_bids,
                    "Player {}: expected {} pending bids, got {}",
                    player_idx, expected_bids, actual
                );
            }

            if let Some(expected_asks) = expected.asks {
                let actual = state.pending_asks.len();
                assert_eq!(
                    actual, expected_asks,
                    "Player {}: expected {} pending asks, got {}",
                    player_idx, expected_asks, actual
                );
            }

            self
        }

        #[track_caller]
        fn check_outcome(
            &self,
            expected: ExpectedOutcome,
        ) -> &Self {
            match (&self.last_result, &expected) {
                (Ok(_), ExpectedOutcome::Ok) => {}
                (
                    Err(GameError::InsufficientFunds { available, required }),
                    ExpectedOutcome::InsufficientFunds {
                        available: exp_avail,
                        required: exp_req,
                    },
                ) => {
                    assert_eq!(*available, *exp_avail, "InsufficientFunds: available mismatch");
                    assert_eq!(*required, *exp_req, "InsufficientFunds: required mismatch");
                }
                (
                    Err(GameError::InsufficientShares { available, required }),
                    ExpectedOutcome::InsufficientShares {
                        available: exp_avail,
                        required: exp_req,
                    },
                ) => {
                    assert_eq!(*available, *exp_avail, "InsufficientShares: available mismatch");
                    assert_eq!(*required, *exp_req, "InsufficientShares: required mismatch");
                }
                (Err(GameError::InvalidPhase { action, .. }), ExpectedOutcome::InvalidPhase { action: exp_action }) => {
                    assert_eq!(*action, *exp_action, "InvalidPhase: action mismatch");
                }
                _ => {
                    panic!("Outcome mismatch: expected {:?}, got {:?}", expected, self.last_result);
                }
            }
            self
        }

        #[track_caller]
        fn check_ok(&self) -> &Self {
            self.check_outcome(ExpectedOutcome::Ok)
        }

        #[track_caller]
        fn check_phase(
            &self,
            expected: GamePhase,
        ) -> &Self {
            assert_eq!(
                self.game.phase, expected,
                "Expected phase {:?}, got {:?}",
                expected, self.game.phase
            );
            self
        }

        #[track_caller]
        fn check_price(
            &self,
            expected: i32,
        ) -> &Self {
            assert_eq!(
                self.game.current_price, expected,
                "Expected price {}, got {}",
                expected, self.game.current_price
            );
            self
        }

        #[track_caller]
        fn check_price_in_range(
            &self,
            base: i32,
            max_delta: i32,
        ) -> &Self {
            let delta = (self.game.current_price - base).abs();
            assert!(
                delta <= max_delta,
                "Price {} is outside range [{}, {}]",
                self.game.current_price,
                base - max_delta,
                base + max_delta
            );
            self
        }

        #[track_caller]
        fn check_all_notified<F>(
            &self,
            predicate: F,
        ) -> &Self
        where
            F: Fn(&GameEvent) -> bool,
        {
            let effects = self.last_result.as_ref().expect("last action failed");
            for player_id in &self.players {
                let found = effects.iter().any(|e| match e {
                    GameEffect::Notify { player_id: pid, event } if pid == player_id => predicate(event),
                    _ => false,
                });
                assert!(found, "Player {:?} was not notified", player_id);
            }
            self
        }

        #[track_caller]
        fn check_has_delayed_action(
            &self,
            delay: Duration,
            action: GameAction,
        ) -> &Self {
            let effects = self.last_result.as_ref().expect("last action failed");
            let found = effects.iter().any(|e| {
                matches!(e, GameEffect::DelayedAction { delay: d, action: a } if *d == delay && matches!((a, &action), (GameAction::Tick, GameAction::Tick) | (GameAction::End, GameAction::End)))
            });
            assert!(found, "Expected DelayedAction {{ delay: {:?}, action: {:?} }}", delay, action);
            self
        }
    }

    fn test_config() -> GameConfig {
        GameConfig {
            tick_interval: Duration::from_secs(1),
            game_duration: Duration::from_secs(10),
            max_price_delta: 10,
            starting_price: 50,
            countdown_duration: Duration::from_secs(3),
            starting_balance: 100,
        }
    }

    #[test]
    fn test_transactions() {
        let mut t = TestHarness::new(1).at_price(0);

        // Place 3 bids totaling 100 (all available cash)
        t.bid(0, 20).bid(0, 40).bid(0, 40);
        t.check(0, player().cash(0).bids(3));

        // Resolve at price 30: two 40-bids fill, one 20-bid stays pending
        t.set_price(30).resolve_bids();
        t.check(0, player().cash(20).shares(2).bids(1));

        // Place an ask
        t.ask(0, 75);
        t.check(0, player().asks(1));

        // Resolve at price 100: ask fills, player gets 100 cash
        t.set_price(100).resolve_asks();
        t.check(0, player().cash(120).shares(1).asks(0));
    }

    #[test]
    fn test_bid_insufficient_funds() {
        let mut t = TestHarness::new(1).at_price(50);

        // Try to bid more than available (100 starting balance)
        t.bid(0, 150);
        t.check_outcome(ExpectedOutcome::InsufficientFunds {
            available: 100,
            required: 150,
        });
    }

    #[test]
    fn test_ask_insufficient_shares() {
        let mut t = TestHarness::new(1).at_price(50);

        // Try to ask without owning any shares
        t.ask(0, 50);
        t.check_outcome(ExpectedOutcome::InsufficientShares {
            available: 0,
            required: 1,
        });
    }

    #[test]
    fn test_start_game() {
        let mut t = TestHarness::new(2).pending();

        t.check_phase(GamePhase::Pending);

        t.start();
        t.check_ok()
            .check_phase(GamePhase::Running)
            .check_price(50)
            .check_all_notified(|e| matches!(e, GameEvent::GameStarted { .. }))
            .check_has_delayed_action(Duration::from_secs(1), GameAction::Tick);
    }

    #[test]
    fn test_price_tick() {
        let mut t = TestHarness::new(1).at_price(50);

        t.tick();
        t.check_ok()
            .check_price_in_range(50, 10)
            .check_all_notified(|e| matches!(e, GameEvent::PriceChanged(_)));
    }

    #[test]
    fn test_bid_resolved_notifications() {
        let mut t = TestHarness::new(1).at_price(0);

        t.bid(0, 40);
        t.check(0, player().bids(1));

        // Resolve at price 30 (bid >= price)
        t.set_price(30).resolve_bids();
        t.check(0, player().shares(1).bids(0));
    }

    #[test]
    fn test_ask_resolved_notifications() {
        let mut t = TestHarness::new(1).at_price(50);

        // Buy a share first
        t.bid(0, 50).resolve_bids();
        t.check(0, player().shares(1));

        // Place an ask
        t.ask(0, 60);
        t.check(0, player().asks(1));

        // Resolve at price 70 (ask <= price)
        t.set_price(70).resolve_asks();
        t.check(0, player().shares(0).asks(0));
    }

    #[test]
    fn test_bid_placed_notifications() {
        let mut t = TestHarness::new(2).at_price(50);

        t.bid(0, 50);
        t.check_ok().check_all_notified(|e| matches!(e, GameEvent::BidPlaced { .. }));
    }

    #[test]
    fn test_ask_placed_notifications() {
        let mut t = TestHarness::new(2).at_price(50);

        // Player 0 needs to own a share first
        t.bid(0, 50).resolve_bids();

        t.ask(0, 60);
        t.check_ok().check_all_notified(|e| matches!(e, GameEvent::AskPlaced { .. }));
    }

    #[test]
    fn test_game_end_notifications() {
        let mut t = TestHarness::new(2).at_price(50);

        t.end();
        t.check_ok().check_all_notified(|e| matches!(e, GameEvent::GameEnded));
    }

    #[test]
    fn test_ask_error_when_insufficient_shares() {
        let mut t = TestHarness::new(1).at_price(50);

        // Buy one share
        t.bid(0, 50).resolve_bids();
        t.check(0, player().shares(1));

        // First ask should succeed
        t.ask(0, 60);
        t.check_ok().check(0, player().asks(1));

        // Second ask should fail - only 1 share but already 1 pending ask
        t.ask(0, 70);
        t.check_outcome(ExpectedOutcome::InsufficientShares {
            available: 0,
            required: 1,
        });
        t.check(0, player().asks(1)); // Still just 1 ask
    }

    #[test]
    fn test_bid_error_when_not_running() {
        let mut t = TestHarness::new(1).pending();

        t.bid(0, 50);
        t.check_outcome(ExpectedOutcome::InvalidPhase { action: "Bid" });
    }

    #[test]
    fn test_price_stays_non_negative() {
        let mut t = TestHarness::new(1).at_price(0);

        // Run many ticks to test that price never goes negative
        for _ in 0..100 {
            t.tick();
            assert!(
                t.game.current_price >= 0,
                "Price should never be negative, got {}",
                t.game.current_price
            );
        }
    }
}
