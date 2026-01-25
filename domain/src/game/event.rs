use crate::PlayerId;

#[derive(Clone, Debug)]
pub enum GameEvent {
    Countdown(u32),
    GameStarted {
        starting_price: i32,
        starting_balance: i32,
        players: Vec<PlayerId>,
        game_duration_secs: u64,
    },
    PriceChanged {
        player_id: PlayerId,
        price: i32,
    },
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
    BidCanceled {
        player_id: PlayerId,
        price: i32,
    },
    AskCanceled {
        player_id: PlayerId,
        price: i32,
    },
    GameEnded {
        final_balances: Vec<(PlayerId, i32)>,
    },
}
