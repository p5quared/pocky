#[derive(Debug, PartialEq, Clone, Copy)]
pub struct PlayerId(pub uuid::Uuid);

#[derive(Clone)]
pub struct GameState {
    current_price: i32,
    players: Vec<PlayerId>,

    // Cash balance transactions
    liquid_transactions: Vec<(PlayerId, i32)>,
    open_bids: Vec<(PlayerId, i32)>,

    // Prices at which a share was bought/sold
    owned_shares: Vec<(PlayerId, i32)>,
    open_asks: Vec<(PlayerId, i32)>,
}

#[derive(Clone, Copy)]
pub enum GameAction {
    SetPrice(i32),
    Bid { player_id: PlayerId, bid_value: i32 },
    Ask { player_id: PlayerId, ask_value: i32 },
}

#[derive(Clone, Copy)]
pub enum GameEvent {
    PriceChanged(i32),
    BidResolved { player_id: PlayerId, bid_value: i32 },
    AskResolved { player_id: PlayerId, ask_value: i32 },
    BidRejected { player_id: PlayerId, bid_value: i32 },
    AskRejected { player_id: PlayerId, ask_value: i32 },
}

#[derive(Clone, Copy)]
pub enum GameEffect {
    Notify { player_id: PlayerId, event: GameEvent },
}

impl GameState {
    pub fn process_action(
        &mut self,
        action: GameAction,
    ) -> Vec<GameEffect> {
        match action {
            GameAction::SetPrice(price) => self.handle_price(price),
            GameAction::Bid { player_id, bid_value } => self.handle_bid(player_id, bid_value),
            GameAction::Ask { player_id, ask_value } => self.handle_ask(player_id, ask_value),
        }
    }
}

impl GameState {
    pub fn init(
        players: Vec<PlayerId>,
        starting_balance: i32,
    ) -> Self {
        Self {
            liquid_transactions: players.clone().into_iter().map(|pid| (pid, starting_balance)).collect(),
            players,
            owned_shares: Vec::new(),
            open_bids: Vec::new(),
            open_asks: Vec::new(),
            current_price: 0,
        }
    }

    fn handle_price(
        &mut self,
        price: i32,
    ) -> Vec<GameEffect> {
        self.current_price = price;

        let resolved_bids = self.resolve_bids();
        let resolved_asks = self.resolve_asks();

        let price_notifications = self.players.iter().map(|&player_id| GameEffect::Notify {
            player_id,
            event: GameEvent::PriceChanged(price),
        });

        let bid_notifications = resolved_bids.into_iter().map(|(player_id, bid_value)| GameEffect::Notify {
            player_id,
            event: GameEvent::BidResolved { player_id, bid_value },
        });

        let ask_notifications = resolved_asks.into_iter().map(|(player_id, ask_value)| GameEffect::Notify {
            player_id,
            event: GameEvent::AskResolved { player_id, ask_value },
        });

        price_notifications
            .chain(bid_notifications)
            .chain(ask_notifications)
            .collect()
    }

    fn resolve_bids(&mut self) -> Vec<(PlayerId, i32)> {
        self.open_bids
            .extract_if(.., |&mut (_, v)| v >= self.current_price)
            .map(|(player_id, bid_value)| {
                self.owned_shares.push((player_id, self.current_price));
                self.liquid_transactions.push((player_id, bid_value - self.current_price));
                (player_id, bid_value)
            })
            .collect()
    }

    fn get_cash_balance(
        &self,
        player_id: PlayerId,
    ) -> i32 {
        self.liquid_transactions
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
        if bid_value > self.get_cash_balance(player_id) {
            return vec![GameEffect::Notify {
                player_id,
                event: GameEvent::BidRejected { player_id, bid_value },
            }];
        }

        self.liquid_transactions.push((player_id, -bid_value));
        self.open_bids.push((player_id, bid_value));
        vec![]
    }

    fn handle_ask(
        &mut self,
        player_id: PlayerId,
        ask_value: i32,
    ) -> Vec<GameEffect> {
        if !self.owned_shares.iter().any(|(pid, _)| *pid == player_id) {
            return vec![GameEffect::Notify {
                player_id,
                event: GameEvent::AskRejected { player_id, ask_value },
            }];
        }

        self.open_asks.push((player_id, ask_value));
        vec![]
    }

    fn resolve_asks(&mut self) -> Vec<(PlayerId, i32)> {
        self.open_asks
            .extract_if(.., |&mut (_, v)| v <= self.current_price)
            .map(|(player_id, ask_value)| {
                // Ask is <= price, so sell at price
                if let Some(pos) = self.owned_shares.iter().position(|(pid, _)| *pid == player_id) {
                    self.owned_shares.remove(pos);
                }
                self.liquid_transactions.push((player_id, self.current_price));
                (player_id, ask_value)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .owned_shares
            .iter()
            .filter(|(pid, _)| *pid == player_id)
            .map(|(_, balance)| balance)
            .sum::<i32>();
        assert_eq!(
            got_balance, want_total,
            "Expected total share value for player {:?} to be {}, but got {}",
            player_id, want_total, got_balance
        );

        let got_count = state.owned_shares.iter().filter(|(pid, _)| *pid == player_id).count();

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
        let got_total_value = player_bids.clone().sum();

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
        let got_total_value = player_asks.clone().sum();
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
        let mut engine = GameState::init(vec![p], 100);
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

        engine.process_action(GameAction::SetPrice(30));
        // 2 bids for 40 filled @30, refund 10 each
        assert_cash(&engine, p, 20);
        assert_shares(&engine, p, 2, 60);
        assert_open_bids(&engine, p, 1, 20);

        engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 75,
        });
        assert_open_asks(&engine, p, 1, 75);
        engine.process_action(GameAction::SetPrice(100));
        // ask filled @100
        assert_cash(&engine, p, 120);
        assert_shares(&engine, p, 1, 30);
        assert_open_asks(&engine, p, 0, 0);
    }

    #[test]
    fn test_bid_rejection() {
        let valid_player = PlayerId(uuid::Uuid::new_v4());
        let invalid_player = PlayerId(uuid::Uuid::new_v4());
        let mut engine = GameState::init(vec![valid_player], 100);

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
        let mut engine = GameState::init(vec![p], 100);

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
    fn test_price_notifications() {
        let p1 = PlayerId(uuid::Uuid::new_v4());
        let p2 = PlayerId(uuid::Uuid::new_v4());
        let mut engine = GameState::init(vec![p1, p2], 100);

        let effects = engine.process_action(GameAction::SetPrice(50));

        // Should notify both players of the price
        assert_eq!(effects.len(), 2);
        let notified_players: Vec<_> = effects
            .iter()
            .filter_map(|e| match e {
                GameEffect::Notify {
                    player_id,
                    event: GameEvent::PriceChanged(50),
                } => Some(*player_id),
                _ => None,
            })
            .collect();
        assert!(notified_players.contains(&p1));
        assert!(notified_players.contains(&p2));
    }

    #[test]
    fn test_bid_resolved_notifications() {
        let p = PlayerId(uuid::Uuid::new_v4());
        let mut engine = GameState::init(vec![p], 100);

        engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 40,
        });
        let effects = engine.process_action(GameAction::SetPrice(30));

        // Should have price notification + bid resolved notification
        assert_eq!(effects.len(), 2);

        let has_price = effects.iter().any(|e| {
            matches!(
                e,
                GameEffect::Notify {
                    event: GameEvent::PriceChanged(30),
                    ..
                }
            )
        });
        let has_bid_resolved = effects.iter().any(|e| {
            matches!(
                e,
                GameEffect::Notify {
                    player_id,
                    event: GameEvent::BidResolved { player_id: resolved_id, bid_value: 40 },
                } if *player_id == p && *resolved_id == p
            )
        });

        assert!(has_price, "Expected price notification");
        assert!(has_bid_resolved, "Expected bid resolved notification");
    }

    #[test]
    fn test_ask_resolved_notifications() {
        let p = PlayerId(uuid::Uuid::new_v4());
        let mut engine = GameState::init(vec![p], 100);

        // Buy a share first
        engine.process_action(GameAction::Bid {
            player_id: p,
            bid_value: 50,
        });
        engine.process_action(GameAction::SetPrice(50));
        assert_shares(&engine, p, 1, 50);

        // Place an ask
        engine.process_action(GameAction::Ask {
            player_id: p,
            ask_value: 60,
        });

        // Price goes up, ask should be resolved
        let effects = engine.process_action(GameAction::SetPrice(70));

        let has_ask_resolved = effects.iter().any(|e| {
            matches!(
                e,
                GameEffect::Notify {
                    player_id,
                    event: GameEvent::AskResolved { player_id: resolved_id, ask_value: 60 },
                } if *player_id == p && *resolved_id == p
            )
        });

        assert!(has_ask_resolved, "Expected ask resolved notification");
        assert_shares(&engine, p, 0, 0);
    }
}
