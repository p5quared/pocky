use std::time::Duration;

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
            tick_interval: Duration::from_millis(250),
            game_duration: Duration::from_secs(180),
            max_price_delta: 25,
            starting_price: 100,
            countdown_duration: Duration::from_secs(3),
            starting_balance: 1000,
        }
    }
}
