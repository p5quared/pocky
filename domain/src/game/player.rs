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
