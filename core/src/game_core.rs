use thiserror::Error;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct PlayerId(pub uuid::Uuid);

#[derive(Clone)]
pub struct GameStateCore {
    current_price: i32,

    // Cash balance transactions
    liquid_transactions: Vec<(PlayerId, i32)>,
    open_bids: Vec<(PlayerId, i32)>,

    // Prices at which a share was bought/sold
    owned_shares: Vec<(PlayerId, i32)>,
    open_asks: Vec<(PlayerId, i32)>,
}

#[derive(Clone, Copy)]
pub enum GameAction {
    Price(i32),
    Bid { player_id: PlayerId, bid_value: i32 },
    Ask { player_id: PlayerId, ask_value: i32 },
}

#[derive(Debug, Error, PartialEq)]
pub enum GameError {
    #[error("insufficient balance")]
    InsufficientBalance,
}

impl GameStateCore {
    pub fn process_event(
        &mut self,
        action: GameAction,
    ) -> Result<(), GameError> {
        match action {
            GameAction::Price(price) => self.handle_price(price),
            GameAction::Bid { player_id, bid_value } => self.handle_bid(player_id, bid_value),
            GameAction::Ask { player_id, ask_value } => self.handle_ask(player_id, ask_value),
        }
    }
}

impl GameStateCore {
    pub fn init(
        players: Vec<PlayerId>,
        starting_balance: i32,
    ) -> Self {
        Self {
            liquid_transactions: players.into_iter().map(|pid| (pid, starting_balance)).collect(),
            owned_shares: Vec::new(),
            open_bids: Vec::new(),
            open_asks: Vec::new(),
            current_price: 0,
        }
    }

    fn handle_price(
        &mut self,
        price: i32,
    ) -> Result<(), GameError> {
        self.current_price = price;
        self.resolve_bids();
        self.resolve_asks();
        Ok(())
    }

    fn resolve_bids(&mut self) {
        let mut fulfilled_bids = Vec::new();

        for (player_id, bid_value) in &self.open_bids {
            if *bid_value >= self.current_price {
                // bid can be fulfilled

                // we'll be generous and fill at best price
                let difference = *bid_value - self.current_price;

                // Credit the player with the asset at current price
                self.owned_shares.push((*player_id, self.current_price.min(*bid_value)));

                // Refund the difference to the player's cash balance
                self.liquid_transactions.push((*player_id, difference));

                fulfilled_bids.push((*player_id, *bid_value));
            }
        }

        self.open_bids.retain(|bid| !fulfilled_bids.contains(bid));
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
    ) -> Result<(), GameError> {
        if bid_value > self.get_cash_balance(player_id) {
            return Err(GameError::InsufficientBalance);
        }

        // Simply add a deduction transaction and record the open bid
        self.liquid_transactions.push((player_id, -bid_value));
        self.open_bids.push((player_id, bid_value));

        Ok(())
    }

    fn handle_ask(
        &mut self,
        player_id: PlayerId,
        ask_value: i32,
    ) -> Result<(), GameError> {
        // We need to verify the player has an asset to sell at ask_value
        // But they can ask for any price, even if it's below their purchase price
        if !self.owned_shares.iter().any(|(pid, _)| *pid == player_id) {
            return Err(GameError::InsufficientBalance);
        }

        self.open_asks.push((player_id, ask_value));
        Ok(())
    }

    fn resolve_asks(&mut self) {
        let mut fulfilled_asks = Vec::new();

        for (player_id, ask_value) in &self.open_asks {
            if *ask_value <= self.current_price {
                // Remove one owned share for the player
                if let Some(pos) = self.owned_shares.iter().position(|(pid, _)| *pid == *player_id) {
                    self.owned_shares.remove(pos);
                }

                // Pay out the difference to the player's cash balance
                self.liquid_transactions
                    .push((*player_id, self.current_price.max(*ask_value)));

                fulfilled_asks.push((*player_id, *ask_value));
            }
        }

        self.open_asks.retain(|bid| !fulfilled_asks.contains(bid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_cash(
        state: &GameStateCore,
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
        state: &GameStateCore,
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
        state: &GameStateCore,
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
        state: &GameStateCore,
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
        let mut engine = GameStateCore::init(vec![p], 100);
        engine
            .process_event(GameAction::Bid {
                player_id: p,
                bid_value: 20,
            })
            .unwrap();
        engine
            .process_event(GameAction::Bid {
                player_id: p,
                bid_value: 40,
            })
            .unwrap();
        engine
            .process_event(GameAction::Bid {
                player_id: p,
                bid_value: 40,
            })
            .unwrap();

        assert_cash(&engine, p, 0);
        assert_open_bids(&engine, p, 3, 100);

        engine.process_event(GameAction::Price(30)).unwrap();
        // 2 bids for 40 filled @30, refund 10 each
        assert_cash(&engine, p, 20);
        assert_shares(&engine, p, 2, 60);
        assert_open_bids(&engine, p, 1, 20);

        engine
            .process_event(GameAction::Ask {
                player_id: p,
                ask_value: 75,
            })
            .unwrap();
        assert_open_asks(&engine, p, 1, 75);
        engine.process_event(GameAction::Price(100)).unwrap();
        // ask filled @100
        assert_cash(&engine, p, 120);
        assert_shares(&engine, p, 1, 30);
        assert_open_asks(&engine, p, 0, 0);
    }

    #[test]
    fn test_bid_insufficient_balance() {
        let valid_player = PlayerId(uuid::Uuid::new_v4());
        let invalid_player = PlayerId(uuid::Uuid::new_v4());
        let mut engine = GameStateCore::init(vec![valid_player], 100);
        let action = GameAction::Bid {
            player_id: invalid_player,
            bid_value: 50,
        };
        let err = engine.process_event(action).unwrap_err();
        assert_eq!(err, GameError::InsufficientBalance);
    }
}
