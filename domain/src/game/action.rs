use crate::PlayerId;

#[derive(Clone, Copy, Debug)]
pub enum GameAction {
    Countdown(u32),
    Start,
    Tick,
    Bid { player_id: PlayerId, bid_value: i32 },
    Ask { player_id: PlayerId, ask_value: i32 },
    CancelBid { player_id: PlayerId, price: i32 },
    CancelAsk { player_id: PlayerId, price: i32 },
    End,
}
