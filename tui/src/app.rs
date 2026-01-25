use std::collections::HashMap;
use std::time::Instant;

use domain::{GameId, PlayerId};

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
pub enum OrderType {
    Bid,
    Ask,
}

#[derive(Debug, Clone)]
pub struct OpenOrder {
    pub order_type: OrderType,
    pub price: i32,
    pub player_id: PlayerId,
    pub is_own: bool,
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
    pub all_prices: HashMap<PlayerId, i32>,
    pub price_history: Vec<(f64, f64)>,
    pub time_index: usize,
    pub cursor_price: i32,
    pub open_orders: Vec<OpenOrder>,
}

impl GameState {
    pub fn new(
        game_id: GameId,
        starting_price: i32,
        starting_balance: i32,
        players: Vec<PlayerId>,
    ) -> Self {
        let all_prices: HashMap<PlayerId, i32> = players.iter().map(|&p| (p, starting_price)).collect();
        Self {
            phase: GamePhase::Running,
            game_id,
            current_price: starting_price,
            starting_price,
            starting_balance,
            balance: starting_balance,
            shares: 0,
            players,
            all_prices,
            price_history: vec![(0.0, starting_price as f64)],
            time_index: 0,
            cursor_price: starting_price,
            open_orders: Vec::new(),
        }
    }

    pub fn move_cursor_up(&mut self) {
        let (min, max) = self.price_bounds();
        let step = ((max - min) * 0.02).max(1.0) as i32;
        self.cursor_price += step;
    }

    pub fn move_cursor_down(&mut self) {
        let (min, max) = self.price_bounds();
        let step = ((max - min) * 0.02).max(1.0) as i32;
        self.cursor_price = (self.cursor_price - step).max(1);
    }

    pub fn add_order(
        &mut self,
        order: OpenOrder,
    ) {
        self.open_orders.push(order);
    }

    pub fn remove_order(
        &mut self,
        player_id: PlayerId,
        price: i32,
        order_type: OrderType,
    ) {
        if let Some(idx) = self
            .open_orders
            .iter()
            .position(|o| o.player_id == player_id && o.price == price && o.order_type == order_type)
        {
            self.open_orders.remove(idx);
        }
    }

    pub fn set_player_price(
        &mut self,
        player_id: PlayerId,
        price: i32,
        my_player_id: Option<PlayerId>,
    ) {
        self.all_prices.insert(player_id, price);
        if Some(player_id) == my_player_id {
            self.time_index += 1;
            self.current_price = price;
            self.price_history.push((self.time_index as f64, price as f64));
        }
    }

    pub fn price_bounds(&self) -> (f64, f64) {
        let mut min = self.price_history.iter().map(|(_, p)| *p).fold(f64::INFINITY, f64::min);
        let mut max = self.price_history.iter().map(|(_, p)| *p).fold(f64::NEG_INFINITY, f64::max);

        // Include cursor price in bounds
        min = min.min(self.cursor_price as f64);
        max = max.max(self.cursor_price as f64);

        // Include open orders in bounds
        for order in &self.open_orders {
            min = min.min(order.price as f64);
            max = max.max(order.price as f64);
        }

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
        }
    }

    pub fn reset_to_matchmaking(&mut self) {
        self.screen = Screen::Matchmaking;
        self.game = None;
        self.countdown = None;
        self.queue = QueueState::Idle;
        self.queue_players.clear();
        self.matched_players = None;
    }

    pub fn can_buy(&self) -> bool {
        if let Some(ref game) = self.game {
            game.phase == GamePhase::Running && game.balance >= game.cursor_price
        } else {
            false
        }
    }

    pub fn can_sell(&self) -> bool {
        if let Some(ref game) = self.game {
            let own_asks = game
                .open_orders
                .iter()
                .filter(|o| o.is_own && o.order_type == OrderType::Ask)
                .count() as i32;
            game.phase == GamePhase::Running && game.shares > own_asks
        } else {
            false
        }
    }

    pub fn tick(&mut self) {
        self.animation_tick = self.animation_tick.wrapping_add(1);
    }

    pub fn can_join_queue(&self) -> bool {
        matches!(self.connection, ConnectionState::Connected) && matches!(self.queue, QueueState::Idle)
    }

    pub fn can_leave_queue(&self) -> bool {
        matches!(self.connection, ConnectionState::Connected) && matches!(self.queue, QueueState::InQueue)
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
