use domain::{GameId, PlayerId};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueState {
    Idle,
    Joining,
    InQueue,
    Leaving,
    Matched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonFocus {
    JoinQueue,
    LeaveQueue,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Matchmaking,
    Game,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamePhase {
    Countdown(u32),
    Running,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameButtonFocus {
    Buy,
    Sell,
}

pub struct GameState {
    pub phase: GamePhase,
    pub game_id: GameId,
    pub current_price: i32,
    pub starting_price: i32,
    pub starting_balance: i32,
    pub balance: i32,
    pub shares: i32,
    pub players: Vec<PlayerId>,
    pub price_history: Vec<(f64, f64)>,
    pub time_index: usize,
    pub event_log: Vec<String>,
}

impl GameState {
    pub fn new(
        game_id: GameId,
        starting_price: i32,
        starting_balance: i32,
        players: Vec<PlayerId>,
    ) -> Self {
        Self {
            phase: GamePhase::Running,
            game_id,
            current_price: starting_price,
            starting_price,
            starting_balance,
            balance: starting_balance,
            shares: 0,
            players,
            price_history: vec![(0.0, starting_price as f64)],
            time_index: 0,
            event_log: Vec::new(),
        }
    }

    pub fn log_event(&mut self, event: String) {
        self.event_log.push(event);
        // Keep only last 50 events
        if self.event_log.len() > 50 {
            self.event_log.remove(0);
        }
    }

    pub fn add_price(&mut self, price: i32) {
        self.time_index += 1;
        self.current_price = price;
        self.price_history.push((self.time_index as f64, price as f64));
    }

    pub fn price_bounds(&self) -> (f64, f64) {
        let min = self.price_history.iter().map(|(_, p)| *p).fold(f64::INFINITY, f64::min);
        let max = self.price_history.iter().map(|(_, p)| *p).fold(f64::NEG_INFINITY, f64::max);
        let padding = (max - min).max(10.0) * 0.1;
        ((min - padding).max(0.0), max + padding)
    }

    pub fn time_bounds(&self) -> (f64, f64) {
        (0.0, (self.time_index as f64).max(10.0))
    }
}

pub struct App {
    pub connection: ConnectionState,
    pub queue: QueueState,
    pub player_id: Option<PlayerId>,
    pub queue_players: Vec<PlayerId>,
    pub matched_players: Option<Vec<PlayerId>>,
    pub animation_tick: usize,
    pub queue_joined_at: Option<Instant>,
    pub error_message: Option<String>,
    pub should_quit: bool,
    pub selected_button: ButtonFocus,
    pub screen: Screen,
    pub game: Option<GameState>,
    pub countdown: Option<u32>,
    pub game_button: GameButtonFocus,
}

impl App {
    pub fn new() -> Self {
        Self {
            connection: ConnectionState::Disconnected,
            queue: QueueState::Idle,
            player_id: None,
            queue_players: Vec::new(),
            matched_players: None,
            animation_tick: 0,
            queue_joined_at: None,
            error_message: None,
            should_quit: false,
            selected_button: ButtonFocus::JoinQueue,
            screen: Screen::Matchmaking,
            game: None,
            countdown: None,
            game_button: GameButtonFocus::Buy,
        }
    }

    pub fn reset_to_matchmaking(&mut self) {
        self.screen = Screen::Matchmaking;
        self.game = None;
        self.countdown = None;
        self.queue = QueueState::Idle;
        self.queue_players.clear();
        self.matched_players = None;
        self.game_button = GameButtonFocus::Buy;
    }

    pub fn toggle_game_button(&mut self) {
        self.game_button = match self.game_button {
            GameButtonFocus::Buy => GameButtonFocus::Sell,
            GameButtonFocus::Sell => GameButtonFocus::Buy,
        };
    }

    pub fn can_buy(&self) -> bool {
        if let Some(ref game) = self.game {
            game.phase == GamePhase::Running && game.balance >= game.current_price
        } else {
            false
        }
    }

    pub fn can_sell(&self) -> bool {
        if let Some(ref game) = self.game {
            game.phase == GamePhase::Running && game.shares > 0
        } else {
            false
        }
    }

    pub fn tick(&mut self) {
        self.animation_tick = self.animation_tick.wrapping_add(1);
    }

    pub fn can_join_queue(&self) -> bool {
        matches!(self.connection, ConnectionState::Connected)
            && matches!(self.queue, QueueState::Idle)
    }

    pub fn can_leave_queue(&self) -> bool {
        matches!(self.connection, ConnectionState::Connected)
            && matches!(self.queue, QueueState::InQueue)
    }

    pub fn queue_elapsed(&self) -> Option<String> {
        self.queue_joined_at.map(|start| {
            let elapsed = start.elapsed();
            format!("{:02}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
        })
    }

    pub fn next_button(&mut self) {
        self.selected_button = match self.selected_button {
            ButtonFocus::JoinQueue => ButtonFocus::LeaveQueue,
            ButtonFocus::LeaveQueue => ButtonFocus::Quit,
            ButtonFocus::Quit => ButtonFocus::JoinQueue,
        };
    }

    pub fn prev_button(&mut self) {
        self.selected_button = match self.selected_button {
            ButtonFocus::JoinQueue => ButtonFocus::Quit,
            ButtonFocus::LeaveQueue => ButtonFocus::JoinQueue,
            ButtonFocus::Quit => ButtonFocus::LeaveQueue,
        };
    }
}
