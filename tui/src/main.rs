use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;

mod app;
mod events;
mod ui;
mod ws;

use app::{App, ButtonFocus, ConnectionState, GamePhase, GameState, QueueState, Screen};
use events::AppEvent;
use ws::{GameNotification, MatchmakingMessage, OutgoingMessage, ServerMessage};

const TICK_RATE: Duration = Duration::from_millis(100);
const WS_URL: &str = "ws://localhost:3000/ws";

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);
    let (ws_tx, ws_rx) = mpsc::channel::<ws::WsCommand>(100);

    // Spawn input/tick task
    let input_event_tx = event_tx.clone();
    tokio::spawn(async move {
        input_tick_loop(input_event_tx).await;
    });

    // Spawn WebSocket task
    let ws_event_tx = event_tx.clone();
    tokio::spawn(async move {
        ws::websocket_loop(WS_URL, ws_rx, ws_event_tx).await;
    });

    // Initialize app and auto-connect
    let mut app = App::new();
    app.connection = ConnectionState::Connecting;
    let _ = ws_tx.send(ws::WsCommand::Connect).await;

    // Main event loop
    loop {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if let Some(ev) = event_rx.recv().await {
            handle_event(&mut app, ev, &ws_tx).await;
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn input_tick_loop(tx: mpsc::Sender<AppEvent>) {
    let mut last_tick = Instant::now();

    loop {
        let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());

        if crossterm::event::poll(timeout).unwrap_or(false) {
            if let Ok(Event::Key(key)) = crossterm::event::read() {
                if key.kind == KeyEventKind::Press {
                    let _ = tx.send(AppEvent::Key(key)).await;
                }
            }
        }

        if last_tick.elapsed() >= TICK_RATE {
            let _ = tx.send(AppEvent::Tick).await;
            last_tick = Instant::now();
        }
    }
}

async fn handle_event(app: &mut App, ev: AppEvent, ws_tx: &mpsc::Sender<ws::WsCommand>) {
    match ev {
        AppEvent::Key(key) => {
            match app.screen {
                Screen::Matchmaking => handle_matchmaking_key(app, key, ws_tx).await,
                Screen::Game => handle_game_key(app, key, ws_tx).await,
            }
        }
        AppEvent::Tick => {
            app.tick();
        }
        AppEvent::WsConnected => {
            app.connection = ConnectionState::Connected;
            app.error_message = None;
        }
        AppEvent::WsDisconnected => {
            app.connection = ConnectionState::Disconnected;
            app.reset_to_matchmaking();
        }
        AppEvent::WsError(e) => {
            app.connection = ConnectionState::Disconnected;
            app.error_message = Some(e);
        }
        AppEvent::WsMessage(msg) => {
            handle_server_message(app, msg);
        }
    }
}

async fn handle_matchmaking_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    ws_tx: &mpsc::Sender<ws::WsCommand>,
) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Enter => match app.selected_button {
            ButtonFocus::JoinQueue if app.can_join_queue() => {
                app.queue = QueueState::Joining;
                app.error_message = None;
                let _ = ws_tx.send(ws::WsCommand::Send(OutgoingMessage::JoinQueue)).await;
            }
            ButtonFocus::LeaveQueue if app.can_leave_queue() => {
                app.queue = QueueState::Leaving;
                app.error_message = None;
                let _ = ws_tx.send(ws::WsCommand::Send(OutgoingMessage::LeaveQueue)).await;
            }
            ButtonFocus::Quit => {
                app.should_quit = true;
            }
            _ => {}
        },
        KeyCode::Tab | KeyCode::Down | KeyCode::Right => {
            app.next_button();
        }
        KeyCode::BackTab | KeyCode::Up | KeyCode::Left => {
            app.prev_button();
        }
        _ => {}
    }
}

async fn handle_game_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    ws_tx: &mpsc::Sender<ws::WsCommand>,
) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if let Some(ref game) = app.game {
                if game.phase == GamePhase::Ended {
                    app.reset_to_matchmaking();
                    return;
                }
            }
            app.should_quit = true;
        }
        KeyCode::Char('b') | KeyCode::Char('B') => {
            if app.can_buy() {
                if let Some(ref game) = app.game {
                    let _ = ws_tx
                        .send(ws::WsCommand::Send(OutgoingMessage::PlaceBid {
                            game_id: game.game_id,
                            value: game.current_price,
                        }))
                        .await;
                }
            }
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            if app.can_sell() {
                if let Some(ref game) = app.game {
                    let _ = ws_tx
                        .send(ws::WsCommand::Send(OutgoingMessage::PlaceAsk {
                            game_id: game.game_id,
                            value: game.current_price,
                        }))
                        .await;
                }
            }
        }
        KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
            app.toggle_game_button();
        }
        KeyCode::Enter => {
            if app.game_button == app::GameButtonFocus::Buy && app.can_buy() {
                if let Some(ref game) = app.game {
                    let _ = ws_tx
                        .send(ws::WsCommand::Send(OutgoingMessage::PlaceBid {
                            game_id: game.game_id,
                            value: game.current_price,
                        }))
                        .await;
                }
            } else if app.game_button == app::GameButtonFocus::Sell && app.can_sell() {
                if let Some(ref game) = app.game {
                    let _ = ws_tx
                        .send(ws::WsCommand::Send(OutgoingMessage::PlaceAsk {
                            game_id: game.game_id,
                            value: game.current_price,
                        }))
                        .await;
                }
            }
        }
        _ => {}
    }
}

fn handle_server_message(app: &mut App, msg: ServerMessage) {
    match msg {
        ServerMessage::Game(notification) => {
            handle_game_notification(app, notification);
        }
        ServerMessage::Matchmaking(mm_msg) => {
            handle_matchmaking_message(app, mm_msg);
        }
    }
}

fn handle_game_notification(app: &mut App, notification: GameNotification) {
    match notification {
        GameNotification::Countdown { remaining, .. } => {
            app.countdown = Some(remaining);
            if app.screen == Screen::Matchmaking {
                app.screen = Screen::Game;
            }
        }
        GameNotification::GameStarted {
            game_id,
            starting_price,
            starting_balance,
            players,
        } => {
            app.countdown = None;
            app.game = Some(GameState::new(game_id, starting_price, starting_balance, players));
            if let Some(ref mut game) = app.game {
                game.log_event("Game started!".to_string());
            }
        }
        GameNotification::PriceChanged { price, .. } => {
            if let Some(ref mut game) = app.game {
                game.add_price(price);
            }
        }
        GameNotification::BidPlaced { player_id, bid_value, .. } => {
            if let Some(ref mut game) = app.game {
                let is_self = app.player_id == Some(player_id);
                let msg = if is_self {
                    format!("You placed bid at ${}", bid_value)
                } else {
                    let short_id = &player_id.0.to_string()[..8];
                    format!("Player {}... placed bid at ${}", short_id, bid_value)
                };
                game.log_event(msg);
            }
        }
        GameNotification::AskPlaced { player_id, ask_value, .. } => {
            if let Some(ref mut game) = app.game {
                let is_self = app.player_id == Some(player_id);
                let msg = if is_self {
                    format!("You placed ask at ${}", ask_value)
                } else {
                    let short_id = &player_id.0.to_string()[..8];
                    format!("Player {}... placed ask at ${}", short_id, ask_value)
                };
                game.log_event(msg);
            }
        }
        GameNotification::BidFilled { player_id, bid_value, .. } => {
            if let Some(ref mut game) = app.game {
                let is_self = app.player_id == Some(player_id);
                if is_self {
                    game.balance -= bid_value;
                    game.shares += 1;
                    game.log_event(format!("Your bid filled at ${}", bid_value));
                } else {
                    let short_id = &player_id.0.to_string()[..8];
                    game.log_event(format!("Player {}... bid filled at ${}", short_id, bid_value));
                }
            }
        }
        GameNotification::AskFilled { player_id, ask_value, .. } => {
            if let Some(ref mut game) = app.game {
                let is_self = app.player_id == Some(player_id);
                if is_self {
                    game.balance += ask_value;
                    game.shares -= 1;
                    game.log_event(format!("Your ask filled at ${}", ask_value));
                } else {
                    let short_id = &player_id.0.to_string()[..8];
                    game.log_event(format!("Player {}... ask filled at ${}", short_id, ask_value));
                }
            }
        }
        GameNotification::GameEnded { .. } => {
            if let Some(ref mut game) = app.game {
                game.phase = GamePhase::Ended;
                game.log_event("Game ended!".to_string());
            }
        }
    }
}

fn handle_matchmaking_message(app: &mut App, msg: MatchmakingMessage) {
    match msg {
        MatchmakingMessage::Enqueued(player_id) => {
            if !app.queue_players.contains(&player_id) {
                app.queue_players.push(player_id);
            }
            if matches!(app.queue, QueueState::Joining) {
                app.queue = QueueState::InQueue;
                app.player_id = Some(player_id);
                app.queue_joined_at = Some(Instant::now());
            }
        }
        MatchmakingMessage::Dequeued(player_id) => {
            app.queue_players.retain(|p| *p != player_id);
            if app.player_id == Some(player_id) {
                app.queue = QueueState::Idle;
                app.player_id = None;
                app.queue_joined_at = None;
            }
        }
        MatchmakingMessage::Matched(players) => {
            app.matched_players = Some(players);
            app.queue = QueueState::Matched;
            app.queue_players.clear();
        }
        MatchmakingMessage::AlreadyQueued => {
            app.error_message = Some("Already in queue".to_string());
            app.queue = QueueState::InQueue;
        }
        MatchmakingMessage::PlayerNotFound => {
            app.error_message = Some("Player not found in queue".to_string());
            app.queue = QueueState::Idle;
        }
    }
}
