use std::time::Duration;

use crate::*;

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
        for player_ticker in self.game.player_tickers.values_mut() {
            player_ticker.current_price = price;
        }
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
        for player_ticker in self.game.player_tickers.values_mut() {
            player_ticker.current_price = price;
        }
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
        for (&player_id, player_ticker) in &self.game.player_tickers {
            assert_eq!(
                player_ticker.current_price, expected,
                "Player {:?}: expected price {}, got {}",
                player_id, expected, player_ticker.current_price
            );
        }
        self
    }

    #[track_caller]
    fn check_price_in_range(
        &self,
        base: i32,
        max_delta: i32,
    ) -> &Self {
        for (&player_id, player_ticker) in &self.game.player_tickers {
            let delta = (player_ticker.current_price - base).abs();
            assert!(
                delta <= max_delta,
                "Player {:?}: Price {} is outside range [{}, {}]",
                player_id,
                player_ticker.current_price,
                base - max_delta,
                base + max_delta
            );
        }
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
                GameEffect::Notification { player_id: pid, event } if pid == player_id => predicate(event),
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
        .check_all_notified(|e| matches!(e, GameEvent::PriceChanged { .. }));
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
        // Check all players' prices stay non-negative
        for (&player_id, player_ticker) in &t.game.player_tickers {
            assert!(
                player_ticker.current_price >= 0,
                "Player {:?}: Price should never be negative, got {}",
                player_id,
                player_ticker.current_price
            );
        }
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
