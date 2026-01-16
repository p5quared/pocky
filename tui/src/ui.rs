use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph},
};

use crate::app::{App, ButtonFocus, ConnectionState, GameButtonFocus, GamePhase, QueueState, Screen};

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
    let title = Paragraph::new("MATCHMAKING")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM));
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

    let (status_text, status_color) = match &app.connection {
        ConnectionState::Disconnected => ("● Disconnected", Color::Red),
        ConnectionState::Connecting => ("◐ Connecting...", Color::Yellow),
        ConnectionState::Connected => ("● Connected", Color::Green),
    };

    let status = Paragraph::new(format!("Status: {}", status_text))
        .style(Style::default().fg(status_color))
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(status, chunks[0]);

    if let Some(elapsed) = app.queue_elapsed() {
        let time = Paragraph::new(format!("Queue Time: {}", elapsed))
            .style(Style::default().fg(Color::White))
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
        format!(" Players in Queue ({}) {} ", app.queue_players.len(), spinner)
    } else {
        " Players in Queue ".to_string()
    };

    let items: Vec<ListItem> = app
        .queue_players
        .iter()
        .enumerate()
        .map(|(i, player_id)| {
            let is_self = app.player_id == Some(*player_id);
            let suffix = if is_self { " (you)" } else { "" };
            let uuid_str = player_id.0.to_string();
            let short_id = &uuid_str[..8];
            let text = format!("{}. {}...{}", i + 1, short_id, suffix);
            let style = if is_self {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
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
        format!("Joining... {}", spinner)
    } else {
        "Join Queue".to_string()
    };
    render_button(frame, chunks[0], &join_text, join_selected, join_enabled);

    // Leave Queue button
    let leave_enabled = app.can_leave_queue();
    let leave_selected = app.selected_button == ButtonFocus::LeaveQueue;
    let leave_text = if matches!(app.queue, QueueState::Leaving) {
        format!("Leaving... {}", spinner)
    } else {
        "Leave Queue".to_string()
    };
    render_button(frame, chunks[1], &leave_text, leave_selected, leave_enabled);

    // Quit button
    let quit_selected = app.selected_button == ButtonFocus::Quit;
    render_button(frame, chunks[2], "Quit", quit_selected, true);
}

fn render_button(
    frame: &mut Frame,
    area: Rect,
    text: &str,
    selected: bool,
    enabled: bool,
) {
    let style = if !enabled {
        Style::default().fg(Color::DarkGray)
    } else if selected {
        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let border_style = if selected && enabled {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
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
    let text = if let Some(ref error) = app.error_message {
        Paragraph::new(format!("Error: {}", error)).style(Style::default().fg(Color::Red))
    } else if matches!(app.queue, QueueState::Matched) {
        Paragraph::new("Match found! Starting game...").style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Paragraph::new("Press Enter to select, Tab to navigate, Q to quit").style(Style::default().fg(Color::DarkGray))
    };

    frame.render_widget(text.alignment(Alignment::Center), area);
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
        format!("GAME STARTING IN {}...", countdown)
    } else if let Some(ref game) = app.game {
        match game.phase {
            GamePhase::Running => "TRADING".to_string(),
            GamePhase::Ended => "GAME OVER".to_string(),
            GamePhase::Countdown(n) => format!("STARTING IN {}...", n),
        }
    } else {
        "LOADING...".to_string()
    };

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, area);
}

fn render_price_chart(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let (data, x_bounds, y_bounds) = if let Some(ref game) = app.game {
        let x_bounds = game.time_bounds();
        let y_bounds = game.price_bounds();
        (game.price_history.clone(), x_bounds, y_bounds)
    } else {
        (vec![(0.0, 100.0)], (0.0, 10.0), (50.0, 150.0))
    };

    let datasets = vec![
        Dataset::default()
            .name("Price")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&data),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(" Price Chart ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .x_axis(
            Axis::default()
                .title("Time")
                .style(Style::default().fg(Color::Gray))
                .bounds([x_bounds.0, x_bounds.1])
                .labels([format!("{:.0}", x_bounds.0), format!("{:.0}", x_bounds.1)]),
        )
        .y_axis(
            Axis::default()
                .title("Price")
                .style(Style::default().fg(Color::Gray))
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
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    if let Some(ref game) = app.game {
        let price_text = format!("${}", game.current_price);
        let price = Paragraph::new(price_text)
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" Price "));
        frame.render_widget(price, chunks[0]);

        let balance_text = format!("${}", game.balance);
        let balance = Paragraph::new(balance_text)
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" Balance "));
        frame.render_widget(balance, chunks[1]);

        let shares_text = format!("{}", game.shares);
        let shares = Paragraph::new(shares_text)
            .style(Style::default().fg(Color::Magenta))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" Shares "));
        frame.render_widget(shares, chunks[2]);
    } else {
        let waiting = Paragraph::new("Waiting for game to start...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(waiting, area);
    }
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
                let style = if event.contains("filled") {
                    Style::default().fg(Color::Green)
                } else if event.contains("placed") {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(format!("> {}", event)).style(style)
            })
            .collect()
    } else {
        vec![]
    };

    let list = List::new(events).block(
        Block::default()
            .title(" Event Log ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)),
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
    render_button(frame, chunks[0], "BUY (B)", buy_selected, buy_enabled);

    let sell_enabled = app.can_sell();
    let sell_selected = app.game_button == GameButtonFocus::Sell;
    render_button(frame, chunks[1], "SELL (S)", sell_selected, sell_enabled);
}

fn render_game_footer(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let text = if let Some(ref game) = app.game {
        match game.phase {
            GamePhase::Ended => "Press Q to return to matchmaking",
            GamePhase::Running => "B to buy, S to sell, Tab to switch, Q to quit",
            GamePhase::Countdown(_) => "Get ready!",
        }
    } else if app.countdown.is_some() {
        "Get ready!"
    } else {
        ""
    };

    let footer = Paragraph::new(text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, area);
}
