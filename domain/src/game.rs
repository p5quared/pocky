use std::collections::HashMap;
use std::time::Duration;

use rand::Rng;
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
    open_bids: Vec<i32>,
    open_asks: Vec<i32>,
}

impl PlayerState {
    fn new(starting_cash: i32) -> Self {
        Self {
            cash: starting_cash,
            shares: Vec::new(),
            open_bids: Vec::new(),
            open_asks: Vec::new(),
        }
    }

    fn available_cash(&self) -> i32 {
        self.cash - self.open_bids.iter().sum::<i32>()
    }

    fn available_shares(&self) -> usize {
        self.shares.len().saturating_sub(self.open_asks.len())
    }

    fn net_worth(
        &self,
        current_price: i32,
    ) -> i32 {
        self.cash + (self.shares.len() as i32 * current_price)
    }
}

#[derive(Clone, Debug)]
pub struct Ticker {
    base_volatility: i32,
    base_pressure: i32,
    forces: Vec<MarketForce>,
}

impl Ticker {
    pub fn new(base_volatility: i32) -> Self {
        Self {
            base_volatility,
            base_pressure: 0,
            forces: Vec::new(),
        }
    }

    pub fn next_delta(&self) -> i32 {
        let mut rng = rand::thread_rng();
        let conditions = self.compute_conditions();

        let effective_volatility = self.base_volatility + (conditions.volatility * self.base_volatility as f32) as i32;
        let effective_pressure = self.base_pressure + (conditions.pressure * self.base_volatility as f32) as i32;

        rng.gen_range(-effective_volatility..=effective_volatility) + effective_pressure
    }

    pub fn add_force(
        &mut self,
        pressure: f32,
        volatility: f32,
        decay: Decay,
    ) {
        self.forces.push(MarketForce::new(pressure, volatility, decay));
    }

    pub fn compute_conditions(&self) -> MarketConditions {
        let mut conditions = MarketConditions::default();
        for force in &self.forces {
            conditions.pressure += force.effective_pressure();
            conditions.volatility += force.effective_volatility();
        }
        conditions
    }

    pub fn tick(&mut self) {
        for force in &mut self.forces {
            force.decay.tick();
        }
        self.forces.retain(|f| f.decay.strength() > 0.0);
    }

    pub fn on_bid_placed(
        &mut self,
        bid_value: f32,
    ) {
        // Fast bullish
        self.add_force(bid_value / 800.0, 0.0, Decay::linear(5));
        // Slow bearish reversion (90% of fast total)
        self.add_force(-bid_value / 3000.0, 0.0, Decay::linear(20));
    }

    pub fn on_ask_placed(
        &mut self,
        ask_value: f32,
    ) {
        // Fast bearish spike
        self.add_force(-ask_value / 800.0, 0.0, Decay::linear(5));
        // Slow bullish reversion (90% of fast total)
        self.add_force(ask_value / 3000.0, 0.0, Decay::linear(20));
    }

    pub fn on_bid_filled(
        &mut self,
        filled_at: f32,
    ) {
        // Fast bearish (demand consumed) + volatility spike
        self.add_force(-filled_at / 1000.0, 0.08, Decay::linear(4));
        // Slow bullish reversion (full reversion - fills are completed transactions)
        self.add_force(filled_at / 2640.0, 0.0, Decay::linear(18));
    }

    pub fn on_ask_filled(
        &mut self,
        filled_at: f32,
    ) {
        // Fast bullish (supply consumed) + volatility spike
        self.add_force(filled_at / 1000.0, 0.08, Decay::linear(4));
        // Slow bearish reversion (full reversion - fills are completed transactions)
        self.add_force(-filled_at / 2640.0, 0.0, Decay::linear(18));
    }
}

#[derive(Clone, Debug, Default)]
pub struct MarketConditions {
    pub pressure: f32,
    pub volatility: f32,
}

#[derive(Clone, Debug)]
pub enum Decay {
    Instant,
    Duration { remaining: u32 },
    Linear { remaining: u32, initial: u32 },
    Exponential { half_life: f32, age: f32 },
}

impl Decay {
    pub fn duration(ticks: u32) -> Self {
        Decay::Duration { remaining: ticks }
    }

    pub fn linear(ticks: u32) -> Self {
        Decay::Linear {
            remaining: ticks,
            initial: ticks,
        }
    }

    pub fn exponential(half_life: f32) -> Self {
        Decay::Exponential { half_life, age: 0.0 }
    }

    pub fn strength(&self) -> f32 {
        match self {
            Decay::Instant => 1.0,
            Decay::Duration { remaining } => {
                if *remaining > 0 {
                    1.0
                } else {
                    0.0
                }
            }
            Decay::Linear { remaining, initial } => {
                if *initial == 0 {
                    0.0
                } else {
                    *remaining as f32 / *initial as f32
                }
            }
            Decay::Exponential { half_life, age } => {
                if *half_life <= 0.0 {
                    0.0
                } else {
                    0.5_f32.powf(*age / *half_life)
                }
            }
        }
    }

    pub fn tick(&mut self) -> bool {
        match self {
            Decay::Instant => false,
            Decay::Duration { remaining } => {
                *remaining = remaining.saturating_sub(1);
                *remaining > 0
            }
            Decay::Linear { remaining, .. } => {
                *remaining = remaining.saturating_sub(1);
                *remaining > 0
            }
            Decay::Exponential { age, .. } => {
                *age += 1.0;
                self.strength() > 0.01
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct MarketForce {
    pub pressure: f32,
    pub volatility: f32,
    pub decay: Decay,
}

impl MarketForce {
    pub fn new(
        pressure: f32,
        volatility: f32,
        decay: Decay,
    ) -> Self {
        Self {
            pressure,
            volatility,
            decay,
        }
    }

    pub fn effective_pressure(&self) -> f32 {
        self.pressure * self.decay.strength()
    }

    pub fn effective_volatility(&self) -> f32 {
        self.volatility * self.decay.strength()
    }
}

#[derive(Clone)]
pub struct GameState {
    phase: GamePhase,
    config: GameConfig,
    current_price: i32,
    players: HashMap<PlayerId, PlayerState>,
    ticks_remaining: u32,
    ticker: Ticker,
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

#[derive(Clone, Debug)]
pub enum GameEvent {
    Countdown(u32),
    GameStarted {
        starting_price: i32,
        starting_balance: i32,
        players: Vec<PlayerId>,
    },
    PriceChanged(i32),
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
    GameEnded {
        final_balances: Vec<(PlayerId, i32)>,
    },
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
        let players = players
            .into_iter()
            .map(|pid| (pid, PlayerState::new(starting_balance)))
            .collect();
        let ticker = Ticker::new(config.max_price_delta);
        Self {
            phase: GamePhase::Pending,
            config,
            players,
            current_price: 0,
            ticks_remaining: tick_count,
            ticker,
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
        self.require_phase(GamePhase::Pending, "Start")?;

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

        self.ticker.tick();
        self.current_price = (self.current_price + self.ticker.next_delta()).max(0);

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
            .map(|(&player_id, state)| (player_id, state.net_worth(self.current_price)))
            .collect();

        Ok(self
            .players
            .keys()
            .map(|&player_id| GameEffect::Notify {
                player_id,
                event: GameEvent::GameEnded {
                    final_balances: final_balances.clone(),
                },
            })
            .collect())
    }

    fn resolve_bids(&mut self) -> Vec<(PlayerId, i32)> {
        let current_price = self.current_price;
        let can_fill_bid = |bid: i32| bid >= current_price;

        let mut resolved = Vec::new();
        for (player_id, state) in &mut self.players {
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

        for (_, _) in &resolved {
            self.ticker.on_bid_filled(self.current_price as f32);
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

        self.ticker.on_bid_placed(bid_value as f32);

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

        self.ticker.on_ask_placed(ask_value as f32);

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

        for (_, _) in &resolved {
            self.ticker.on_ask_filled(self.current_price as f32);
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
                let actual = state.open_bids.len();
                assert_eq!(
                    actual, expected_bids,
                    "Player {}: expected {} pending bids, got {}",
                    player_idx, expected_bids, actual
                );
            }

            if let Some(expected_asks) = expected.asks {
                let actual = state.open_asks.len();
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
        t.check_ok().check_all_notified(|e| matches!(e, GameEvent::GameEnded { .. }));
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

        // Run ticks until exhausted (10 ticks in test config)
        while t.game.ticks_remaining > 0 {
            t.tick();
            assert!(
                t.game.current_price >= 0,
                "Price should never be negative, got {}",
                t.game.current_price
            );
        }
    }

    #[test]
    fn test_tick_decrements_remaining() {
        let mut t = TestHarness::new(1).at_price(50);
        let initial_ticks = t.game.ticks_remaining;

        t.tick();
        t.check_ok();

        assert_eq!(
            t.game.ticks_remaining,
            initial_ticks - 1,
            "Tick should decrement ticks_remaining"
        );
    }

    #[test]
    fn test_tick_schedules_next_tick() {
        let mut t = TestHarness::new(1).at_price(50);
        // With 10 ticks remaining, should schedule another Tick
        assert!(t.game.ticks_remaining > 1);

        t.tick();
        t.check_ok()
            .check_has_delayed_action(Duration::from_secs(1), GameAction::Tick);
    }

    #[test]
    fn test_final_tick_schedules_end() {
        let mut t = TestHarness::new(1).at_price(50);
        // Consume all but one tick
        while t.game.ticks_remaining > 1 {
            t.tick();
        }
        assert_eq!(t.game.ticks_remaining, 1);

        t.tick();
        t.check_ok().check_has_delayed_action(Duration::from_secs(1), GameAction::End);
    }

    #[test]
    fn test_tick_with_zero_remaining_fails() {
        let mut t = TestHarness::new(1).at_price(50);
        // Consume all ticks
        while t.game.ticks_remaining > 0 {
            t.tick();
        }

        // Attempting another tick should fail
        t.tick();
        t.check_outcome(ExpectedOutcome::InvalidPhase { action: "PriceTick" });
    }
}
