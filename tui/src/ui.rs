use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph},
};

use crate::app::{App, ButtonFocus, ConnectionState, GameButtonFocus, GamePhase, QueueState, Screen};
use crate::theme;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn draw(
    frame: &mut Frame,
    app: &App,
) {
    match app.screen {
        Screen::Matchmaking => draw_matchmaking(frame, app),
        Screen::Game => draw_game(frame, app),
    }
}

fn draw_matchmaking(
    frame: &mut Frame,
    app: &App,
) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Status bar
            Constraint::Min(10),   // Queue list
            Constraint::Length(5), // Buttons
            Constraint::Length(3), // Help text
        ])
        .split(area);

    render_title(frame, chunks[0]);
    render_status_bar(frame, chunks[1], app);
    render_queue_list(frame, chunks[2], app);
    render_buttons(frame, chunks[3], app);
    render_footer(frame, chunks[4], app);
}

fn render_title(
    frame: &mut Frame,
    area: Rect,
) {
    let title = Paragraph::new("◀ MATCHMAKING ▶")
        .style(Style::default().fg(theme::ORANGE).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme::BORDER_INACTIVE)));
    frame.render_widget(title, area);
}

fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (status_icon, status_text, status_color) = match &app.connection {
        ConnectionState::Disconnected => ("●", "DISCONNECTED", theme::RED),
        ConnectionState::Connecting => ("◐", "CONNECTING...", theme::AMBER),
        ConnectionState::Connected => ("●", "ONLINE", theme::GREEN),
    };

    let status = Paragraph::new(format!("STATUS: {} {}", status_icon, status_text))
        .style(Style::default().fg(status_color).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(status, chunks[0]);

    if let Some(elapsed) = app.queue_elapsed() {
        let time = Paragraph::new(format!("QUEUE: {}", elapsed))
            .style(Style::default().fg(theme::YELLOW_DATA))
            .alignment(Alignment::Right)
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(time, chunks[1]);
    }
}

fn render_queue_list(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let spinner = SPINNER_FRAMES[app.animation_tick % SPINNER_FRAMES.len()];

    let title = if matches!(app.queue, QueueState::InQueue) {
        format!("[ QUEUE ({}) {} ]", app.queue_players.len(), spinner)
    } else {
        format!("[ QUEUE ({}) ]", app.queue_players.len())
    };

    let items: Vec<ListItem> = app
        .queue_players
        .iter()
        .enumerate()
        .map(|(i, player_id)| {
            let is_self = app.player_id == Some(*player_id);
            let prefix = if is_self { "▶" } else { " " };
            let suffix = if is_self { "(YOU)" } else { "" };
            let uuid_str = player_id.0.to_string();
            let short_id = &uuid_str[..8];
            let text = format!("{} {:>2}. {}... {}", prefix, i + 1, short_id, suffix);
            let style = if is_self {
                Style::default().fg(theme::ORANGE_BRIGHT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(title)
            .title_style(Style::default().fg(theme::ORANGE).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme::BORDER_ACTIVE)),
    );

    frame.render_widget(list, area);
}

fn render_buttons(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .margin(1)
        .split(area);

    let spinner = SPINNER_FRAMES[app.animation_tick % SPINNER_FRAMES.len()];

    // Join Queue button
    let join_enabled = app.can_join_queue();
    let join_selected = app.selected_button == ButtonFocus::JoinQueue;
    let join_text = if matches!(app.queue, QueueState::Joining) {
        format!("<ENTER> JOINING... {}", spinner)
    } else {
        "<ENTER> JOIN".to_string()
    };
    render_button(frame, chunks[0], &join_text, join_selected, join_enabled);

    // Leave Queue button
    let leave_enabled = app.can_leave_queue();
    let leave_selected = app.selected_button == ButtonFocus::LeaveQueue;
    let leave_text = if matches!(app.queue, QueueState::Leaving) {
        format!("<L> LEAVING... {}", spinner)
    } else {
        "<L> LEAVE".to_string()
    };
    render_button(frame, chunks[1], &leave_text, leave_selected, leave_enabled);

    // Quit button
    let quit_selected = app.selected_button == ButtonFocus::Quit;
    render_button(frame, chunks[2], "<ESC> QUIT", quit_selected, true);
}

fn render_button(
    frame: &mut Frame,
    area: Rect,
    text: &str,
    selected: bool,
    enabled: bool,
) {
    let style = if !enabled {
        Style::default().fg(theme::TEXT_DIM)
    } else if selected {
        Style::default().fg(Color::Black).bg(theme::ORANGE).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::TEXT_SECONDARY)
    };

    let border_style = if selected && enabled {
        Style::default().fg(theme::ORANGE_BRIGHT)
    } else {
        Style::default().fg(theme::BORDER_INACTIVE)
    };

    let button = Paragraph::new(text)
        .style(style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(border_style));

    frame.render_widget(button, area);
}

fn render_footer(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme::BORDER_INACTIVE));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if let Some(ref error) = app.error_message {
        Paragraph::new(format!("ERROR: {}", error.to_uppercase()))
            .style(Style::default().fg(theme::RED).add_modifier(Modifier::BOLD))
    } else if matches!(app.queue, QueueState::Matched) {
        Paragraph::new(">>> MATCH FOUND - STARTING GAME <<<")
            .style(Style::default().fg(theme::GREEN).add_modifier(Modifier::BOLD))
    } else {
        Paragraph::new("TAB=Navigate | ENTER=Select | ESC=Quit")
            .style(Style::default().fg(theme::TEXT_DIM))
    };

    frame.render_widget(text.alignment(Alignment::Center), inner);
}

fn draw_game(
    frame: &mut Frame,
    app: &App,
) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title/Status
            Constraint::Min(10),   // Chart + Event Log
            Constraint::Length(3), // Info panel
            Constraint::Length(3), // Buttons
            Constraint::Length(2), // Footer/Help
        ])
        .split(area);

    render_game_title(frame, chunks[0], app);

    // Horizontal split for chart and event log
    let chart_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Chart
            Constraint::Percentage(40), // Event Log
        ])
        .split(chunks[1]);

    render_price_chart(frame, chart_area[0], app);
    render_event_log(frame, chart_area[1], app);
    render_game_info(frame, chunks[2], app);
    render_game_buttons(frame, chunks[3], app);
    render_game_footer(frame, chunks[4], app);
}

fn render_game_title(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let title_text = if let Some(countdown) = app.countdown {
        format!("[ STARTING IN {}... ]", countdown)
    } else if let Some(ref game) = app.game {
        match game.phase {
            GamePhase::Running => "◀ TRADING ▶".to_string(),
            GamePhase::Ended => "[ GAME OVER ]".to_string(),
            GamePhase::Countdown(n) => format!("[ STARTING IN {}... ]", n),
        }
    } else {
        "[ LOADING... ]".to_string()
    };

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(theme::ORANGE).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme::BORDER_INACTIVE)));
    frame.render_widget(title, area);
}

fn render_price_chart(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let (data, x_bounds, y_bounds, price_up) = if let Some(ref game) = app.game {
        let x_bounds = game.time_bounds();
        let y_bounds = game.price_bounds();
        let price_up = if game.price_history.len() >= 2 {
            let last = game.price_history.last().map(|(_, p)| *p).unwrap_or(0.0);
            let first = game.price_history.first().map(|(_, p)| *p).unwrap_or(0.0);
            last >= first
        } else {
            true
        };
        (game.price_history.clone(), x_bounds, y_bounds, price_up)
    } else {
        (vec![(0.0, 100.0)], (0.0, 10.0), (50.0, 150.0), true)
    };

    // Line color based on price direction
    let line_color = if price_up { theme::GREEN } else { theme::RED };

    let datasets = vec![
        Dataset::default()
            .name("PRICE")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(line_color))
            .data(&data),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title("[ PRICE CHART ]")
                .title_style(Style::default().fg(theme::ORANGE).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme::BORDER_ACTIVE)),
        )
        .x_axis(
            Axis::default()
                .title("TIME")
                .style(Style::default().fg(theme::TEXT_DIM))
                .bounds([x_bounds.0, x_bounds.1])
                .labels([format!("{:.0}", x_bounds.0), format!("{:.0}", x_bounds.1)]),
        )
        .y_axis(
            Axis::default()
                .title("$")
                .style(Style::default().fg(theme::TEXT_DIM))
                .bounds([y_bounds.0, y_bounds.1])
                .labels([
                    format!("{:.0}", y_bounds.0),
                    format!("{:.0}", (y_bounds.0 + y_bounds.1) / 2.0),
                    format!("{:.0}", y_bounds.1),
                ]),
        );

    frame.render_widget(chart, area);
}

fn render_game_info(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    if let Some(ref game) = app.game {
        // Calculate price change
        let (price_change, price_up) = if game.price_history.len() >= 2 {
            let last = game.price_history.last().map(|(_, p)| *p).unwrap_or(0.0);
            let first = game.price_history.first().map(|(_, p)| *p).unwrap_or(0.0);
            if first > 0.0 {
                let pct = ((last - first) / first) * 100.0;
                (pct, last >= first)
            } else {
                (0.0, true)
            }
        } else {
            (0.0, true)
        };

        let arrow = if price_up { "▲" } else { "▼" };
        let price_color = if price_up { theme::GREEN } else { theme::RED };
        let price_text = format!("${} {} {:.1}%", game.current_price, arrow, price_change.abs());

        render_info_box(frame, chunks[0], "PRICE", &price_text, price_color);

        // Balance
        let balance_text = format!("${}", game.balance);
        render_info_box(frame, chunks[1], "BALANCE", &balance_text, theme::YELLOW_DATA);

        // Shares
        let shares_text = format!("{}", game.shares);
        render_info_box(frame, chunks[2], "SHARES", &shares_text, Color::White);

        // P/L calculation: current value - starting balance
        let current_value = game.balance as i64 + (game.shares as i64 * game.current_price as i64);
        let starting_value = game.starting_balance as i64;
        let pnl = current_value - starting_value;
        let pnl_color = if pnl >= 0 { theme::GREEN } else { theme::RED };
        let pnl_sign = if pnl >= 0 { "+" } else { "" };
        let pnl_text = format!("{}${}", pnl_sign, pnl);
        render_info_box(frame, chunks[3], "P/L", &pnl_text, pnl_color);
    } else {
        let waiting = Paragraph::new("Waiting for game to start...")
            .style(Style::default().fg(theme::TEXT_DIM))
            .alignment(Alignment::Center);
        frame.render_widget(waiting, area);
    }
}

fn render_info_box(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    value_color: Color,
) {
    let block = Block::default()
        .title(format!(" {} ", label))
        .title_style(Style::default().fg(theme::TEXT_DIM))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(theme::BORDER_INACTIVE));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let value_widget = Paragraph::new(value)
        .style(Style::default().fg(value_color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);

    frame.render_widget(value_widget, inner);
}

fn render_event_log(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let events: Vec<ListItem> = if let Some(ref game) = app.game {
        game.event_log
            .iter()
            .rev()
            .take(20)
            .rev()
            .map(|event| {
                let (prefix, style) = if event.contains("filled") {
                    ("✓", Style::default().fg(theme::GREEN))
                } else if event.contains("placed") {
                    ("→", Style::default().fg(theme::AMBER))
                } else if event.contains("started") || event.contains("ended") {
                    ("●", Style::default().fg(theme::ORANGE).add_modifier(Modifier::BOLD))
                } else {
                    ("·", Style::default().fg(theme::TEXT_SECONDARY))
                };
                ListItem::new(format!(" {} {}", prefix, event)).style(style)
            })
            .collect()
    } else {
        vec![]
    };

    let list = List::new(events).block(
        Block::default()
            .title("[ EVENT LOG ]")
            .title_style(Style::default().fg(theme::ORANGE).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme::BORDER_INACTIVE)),
    );

    frame.render_widget(list, area);
}

fn render_game_buttons(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .margin(0)
        .split(area);

    let buy_enabled = app.can_buy();
    let buy_selected = app.game_button == GameButtonFocus::Buy;
    render_button(frame, chunks[0], "<B> BUY", buy_selected, buy_enabled);

    let sell_enabled = app.can_sell();
    let sell_selected = app.game_button == GameButtonFocus::Sell;
    render_button(frame, chunks[1], "<S> SELL", sell_selected, sell_enabled);
}

fn render_game_footer(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let text = if let Some(ref game) = app.game {
        match game.phase {
            GamePhase::Ended => "Q=Return to matchmaking",
            GamePhase::Running => "B=Buy | S=Sell | TAB=Switch | Q=Quit",
            GamePhase::Countdown(_) => "Get ready!",
        }
    } else if app.countdown.is_some() {
        "Get ready!"
    } else {
        ""
    };

    let footer = Paragraph::new(text)
        .style(Style::default().fg(theme::TEXT_DIM))
        .alignment(Alignment::Center);
    frame.render_widget(footer, area);
}
