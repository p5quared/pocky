use rand::Rng;
use serde::Serialize;

use super::PlayerId;

#[derive(Clone, Debug, PartialEq)]
pub enum GamePhase {
    Pending,
    Running,
    Ended,
}

#[derive(Clone)]
pub struct GameConfig {
    pub tick_interval_ms: u64,
    pub max_price_delta: i32,
    pub starting_price: i32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            tick_interval_ms: 1,
            max_price_delta: 25,
            starting_price: 100,
        }
    }
}

#[derive(Clone)]
pub struct GameState {
    phase: GamePhase,
    config: GameConfig,
    current_price: i32,
    players: Vec<PlayerId>,

    // (player_id, amount)
    cash_transactions: Vec<(PlayerId, i32)>,
    // (player_id, share_value)
    share_transactions: Vec<(PlayerId, i32)>,

    open_bids: Vec<(PlayerId, i32)>,
    open_asks: Vec<(PlayerId, i32)>,
}

#[derive(Clone, Copy)]
pub enum GameAction {
    Start,
    PriceTick,
    Bid { player_id: PlayerId, bid_value: i32 },
    Ask { player_id: PlayerId, ask_value: i32 },
    End,
}

#[derive(Clone, Copy, Serialize)]
pub enum GameEvent {
    GameStarted { starting_price: i32 },
    PriceChanged(i32),
    BidPlaced { player_id: PlayerId, bid_value: i32 },
    AskPlaced { player_id: PlayerId, ask_value: i32 },
    BidResolved { player_id: PlayerId, bid_value: i32 },
    AskResolved { player_id: PlayerId, ask_value: i32 },
    BidRejected { player_id: PlayerId, bid_value: i32 },
    AskRejected { player_id: PlayerId, ask_value: i32 },
    GameEnded,
}

#[derive(Clone, Copy)]
pub enum GameEffect {
    Notify { player_id: PlayerId, event: GameEvent },
    SchedulePriceTick { delay_ms: u64 },
}

impl GameState {
    pub fn process_action(
        &mut self,
        action: GameAction,
    ) -> Vec<GameEffect> {
        match action {
            GameAction::Start => self.handle_start(),
            GameAction::PriceTick => self.handle_price_tick(),
            GameAction::Bid { player_id, bid_value } => self.handle_bid(player_id, bid_value),
            GameAction::Ask { player_id, ask_value } => self.handle_ask(player_id, ask_value),
            GameAction::End => self.handle_game_end(),
        }
    }
}

impl GameState {
    pub fn new(
        players: Vec<PlayerId>,
        starting_balance: i32,
        config: GameConfig,
    ) -> Self {
        Self {
            phase: GamePhase::Pending,
            config,
            cash_transactions: players.clone().into_iter().map(|pid| (pid, starting_balance)).collect(),
            players,
            share_transactions: Vec::new(),
            open_bids: Vec::new(),
            open_asks: Vec::new(),
            current_price: 0,
        }
    }

    fn handle_start(&mut self) -> Vec<GameEffect> {
        if self.phase != GamePhase::Pending {
            return vec![];
        }

        self.phase = GamePhase::Running;
        self.current_price = self.config.starting_price;

        let mut effects: Vec<GameEffect> = self
            .players
            .iter()
            .map(|&player_id| GameEffect::Notify {
                player_id,
                event: GameEvent::GameStarted {
                    starting_price: self.current_price,
                },
            })
            .collect();

        effects.push(GameEffect::SchedulePriceTick {
            delay_ms: self.config.tick_interval_ms,
        });

        effects
    }

    fn handle_price_tick(&mut self) -> Vec<GameEffect> {
        if self.phase != GamePhase::Running {
            return vec![];
        }

        let mut rng = rand::thread_rng();
        let delta = rng.gen_range(-self.config.max_price_delta..=self.config.max_price_delta);
        self.current_price = (self.current_price + delta).max(0);

        let resolved_bids = self.resolve_bids();
        let resolved_asks = self.resolve_asks();

        let price_notifications = self.players.iter().map(|&player_id| GameEffect::Notify {
            player_id,
            event: GameEvent::PriceChanged(self.current_price),
        });

        let bid_notifications = resolved_bids.into_iter().map(|(player_id, bid_value)| GameEffect::Notify {
            player_id,
            event: GameEvent::BidResolved { player_id, bid_value },
        });

        let ask_notifications = resolved_asks.into_iter().map(|(player_id, ask_value)| GameEffect::Notify {
            player_id,
            event: GameEvent::AskResolved { player_id, ask_value },
        });

        let mut effects: Vec<GameEffect> = price_notifications
            .chain(bid_notifications)
            .chain(ask_notifications)
            .collect();

        effects.push(GameEffect::SchedulePriceTick {
            delay_ms: self.config.tick_interval_ms,
        });

        effects
    }

    fn handle_game_end(&mut self) -> Vec<GameEffect> {
        self.phase = GamePhase::Ended;

        self.players
            .iter()
            .map(|&player_id| GameEffect::Notify {
                player_id,
                event: GameEvent::GameEnded,
            })
            .collect()
    }

    fn resolve_bids(&mut self) -> Vec<(PlayerId, i32)> {
        self.open_bids
            .extract_if(.., |&mut (_, v)| v >= self.current_price)
            .map(|(player_id, bid_value)| {
                self.share_transactions.push((player_id, self.current_price));
                self.cash_transactions.push((player_id, bid_value - self.current_price));
                (player_id, bid_value)
            })
            .collect()
    }

    fn get_cash_balance(
        &self,
        player_id: PlayerId,
    ) -> i32 {
        self.cash_transactions
            .iter()
            .filter(|(pid, _)| *pid == player_id)
            .map(|(_, balance)| balance)
            .sum()
    }

    fn handle_bid(
        &mut self,
        player_id: PlayerId,
        bid_value: i32,
    ) -> Vec<GameEffect> {
        if self.phase != GamePhase::Running {
            return vec![GameEffect::Notify {
                player_id,
                event: GameEvent::BidRejected { player_id, bid_value },
            }];
        }

        if bid_value > self.get_cash_balance(player_id) {
            return vec![GameEffect::Notify {
                player_id,
                event: GameEvent::BidRejected { player_id, bid_value },
            }];
        }

        self.cash_transactions.push((player_id, -bid_value));
        self.open_bids.push((player_id, bid_value));

        self.players
            .iter()
            .map(|&pid| GameEffect::Notify {
                player_id: pid,
                event: GameEvent::BidPlaced { player_id, bid_value },
            })
            .collect()
    }

    fn handle_ask(
        &mut self,
        player_id: PlayerId,
        ask_value: i32,
    ) -> Vec<GameEffect> {
        if self.phase != GamePhase::Running {
            return vec![GameEffect::Notify {
                player_id,
                event: GameEvent::AskRejected { player_id, ask_value },
            }];
        }

        let owned_count = self.share_transactions.iter().filter(|(pid, _)| *pid == player_id).count();
        let asking_count = self.open_asks.iter().filter(|(pid, _)| *pid == player_id).count();

        if owned_count <= asking_count {
            return vec![GameEffect::Notify {
                player_id,
                event: GameEvent::AskRejected { player_id, ask_value },
            }];
        }

        self.open_asks.push((player_id, ask_value));

        self.players
            .iter()
            .map(|&pid| GameEffect::Notify {
                player_id: pid,
                event: GameEvent::AskPlaced { player_id, ask_value },
            })
            .collect()
    }

    fn resolve_asks(&mut self) -> Vec<(PlayerId, i32)> {
        self.open_asks
            .extract_if(.., |&mut (_, v)| v <= self.current_price)
            .map(|(player_id, ask_value)| {
                // Ask is <= price, so sell at price
                if let Some(pos) = self.share_transactions.iter().position(|(pid, _)| *pid == player_id) {
                    self.share_transactions.remove(pos);
                }
                self.cash_transactions.push((player_id, self.current_price));
                (player_id, ask_value)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> GameConfig {
        GameConfig {
            tick_interval_ms: 1000,
            max_price_delta: 10,
            starting_price: 50,
        }
    }

    /// Create a game already in Running state at the given price
    fn create_running_game(
        players: Vec<PlayerId>,
        starting_balance: i32,
        price: i32,
    ) -> GameState {
        let mut config = test_config();
        config.starting_price = price;
        let mut game = GameState::new(players, starting_balance, config);
        game.process_action(GameAction::Start);
        game
    }

    fn assert_cash(
        state: &GameState,
        player_id: PlayerId,
        want_balance: i32,
    ) {
        let got_balance = state.get_cash_balance(player_id);

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
        let got_balance = state
            .share_transactions
            .iter()
            .filter(|(pid, _)| *pid == player_id)
            .map(|(_, balance)| balance)
            .sum::<i32>();
        assert_eq!(
            got_balance, want_total,
            "Expected total share value for player {:?} to be {}, but got {}",
            player_id, want_total, got_balance
        );

        let got_count = state.share_transactions.iter().filter(|(pid, _)| *pid == player_id).count();

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
        let player_bids = state
            .open_bids
            .iter()
            .filter(|(pid, _)| *pid == player_id)
            .map(|(_, value)| *value);

        let got_bid_count = player_bids.clone().count();
        let got_total_value: i32 = player_bids.clone().sum();

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
        let player_asks = state
            .open_asks
            .iter()
            .filter(|(pid, _)| *pid == player_id)
            .map(|(_, value)| *value);
        let got_ask_count = player_asks.clone().count();
        let got_total_value: i32 = player_asks.clone().sum();
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
        let mut engine = create_running_game(vec![p], 100, 0);
        engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 20,
        });
        engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 40,
        });
        engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 40,
        });

        assert_cash(&engine, p, 0);
        assert_open_bids(&engine, p, 3, 100);

        // Set price directly and resolve (simulating what PriceTick does)
        engine.current_price = 30;
        engine.resolve_bids();
        // 2 bids for 40 filled @30, refund 10 each
        assert_cash(&engine, p, 20);
        assert_shares(&engine, p, 2, 60);
        assert_open_bids(&engine, p, 1, 20);

        engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 75,
        });
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
    fn test_bid_rejection() {
        let valid_player = PlayerId(uuid::Uuid::new_v4());
        let invalid_player = PlayerId(uuid::Uuid::new_v4());
        let mut engine = create_running_game(vec![valid_player], 100, 50);

        let effects = engine.process_action(GameAction::Bid {
            player_id: invalid_player,
            bid_value: 50,
        });

        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            GameEffect::Notify {
                player_id,
                event: GameEvent::BidRejected { player_id: rejected_id, bid_value: 50 },
            } if player_id == invalid_player && rejected_id == invalid_player
        ));
    }

    #[test]
    fn test_ask_rejection() {
        let p = PlayerId(uuid::Uuid::new_v4());
        let mut engine = create_running_game(vec![p], 100, 50);

        // No shares owned, ask should be rejected
        let effects = engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 50,
        });

        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            GameEffect::Notify {
                player_id,
                event: GameEvent::AskRejected { player_id: rejected_id, ask_value: 50 },
            } if player_id == p && rejected_id == p
        ));
    }

    #[test]
    fn test_start_game() {
        let p1 = PlayerId(uuid::Uuid::new_v4());
        let p2 = PlayerId(uuid::Uuid::new_v4());
        let mut config = test_config();
        config.starting_price = 50;
        let mut engine = GameState::new(vec![p1, p2], 100, config);

        assert_eq!(engine.phase, GamePhase::Pending);

        let effects = engine.process_action(GameAction::Start);

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

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, GameEffect::SchedulePriceTick { delay_ms: 1000 }))
        );
    }

    #[test]
    fn test_price_tick() {
        let p1 = PlayerId(uuid::Uuid::new_v4());
        let mut engine = create_running_game(vec![p1], 100, 50);

        let effects = engine.process_action(GameAction::PriceTick);

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
        assert!(effects.iter().any(|e| matches!(e, GameEffect::SchedulePriceTick { .. })));
    }

    #[test]
    fn test_bid_resolved_notifications() {
        let p = PlayerId(uuid::Uuid::new_v4());
        // Start at price 0 so the bid doesn't immediately resolve
        let mut engine = create_running_game(vec![p], 100, 0);

        engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 40,
        });

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
        let mut engine = create_running_game(vec![p], 100, 50);

        // Buy a share first
        engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 50,
        });
        // Resolve the bid at current price
        engine.resolve_bids();
        assert_shares(&engine, p, 1, 50);

        // Place an ask
        engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 60,
        });

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
        let mut engine = create_running_game(vec![p1, p2], 100, 50);

        let effects = engine.process_action(GameAction::Bid {
            player_id: p1,
            bid_value: 50,
        });

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
        let mut engine = create_running_game(vec![p1, p2], 100, 50);

        // p1 needs to own a share first
        engine.process_action(GameAction::Bid {
            player_id: p1,
            bid_value: 50,
        });
        engine.resolve_bids();

        let effects = engine.process_action(GameAction::Ask {
            player_id: p1,
            ask_value: 60,
        });

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
        let mut engine = create_running_game(vec![p1, p2], 100, 50);

        let effects = engine.process_action(GameAction::End);

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
    fn test_ask_rejected_when_insufficient_shares() {
        let p = PlayerId(uuid::Uuid::new_v4());
        // Start at price 50 so bid resolves immediately
        let mut engine = create_running_game(vec![p], 100, 50);

        // Buy one share
        engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 50,
        });
        engine.resolve_bids();
        assert_shares(&engine, p, 1, 50);

        // First ask should succeed
        let effects = engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 60,
        });
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

        // Second ask should be rejected - only 1 share but already 1 open ask
        let effects = engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 70,
        });
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            GameEffect::Notify {
                player_id,
                event: GameEvent::AskRejected { player_id: rejected_id, ask_value: 70 },
            } if player_id == p && rejected_id == p
        ));
        assert_open_asks(&engine, p, 1, 60);
    }

    #[test]
    fn test_bid_rejected_when_not_running() {
        let p = PlayerId(uuid::Uuid::new_v4());
        let mut engine = GameState::new(vec![p], 100, test_config());

        // Game is in Pending state, bid should be rejected
        let effects = engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 50,
        });

        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            GameEffect::Notify {
                event: GameEvent::BidRejected { .. },
                ..
            }
        ));
    }

    #[test]
    fn test_price_stays_non_negative() {
        let p = PlayerId(uuid::Uuid::new_v4());
        // Start at price 0
        let mut engine = create_running_game(vec![p], 100, 0);

        // Run many ticks to test that price never goes negative
        for _ in 0..100 {
            engine.process_action(GameAction::PriceTick);
            assert!(engine.current_price >= 0, "Price should never be negative");
        }
    }
}
