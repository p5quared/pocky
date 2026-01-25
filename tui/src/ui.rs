use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph},
};

use crate::app::{App, ButtonFocus, ConnectionState, GamePhase, OrderType, QueueState, Screen};
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
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme::BORDER_INACTIVE)),
        );
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
        Style::default()
            .fg(Color::Black)
            .bg(theme::ORANGE)
            .add_modifier(Modifier::BOLD)
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
        Paragraph::new("TAB=Navigate | ENTER=Select | ESC=Quit").style(Style::default().fg(theme::TEXT_DIM))
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
            Constraint::Min(10),   // Chart + sidebar
            Constraint::Length(3), // Info panel
            Constraint::Length(2), // Footer/Help
        ])
        .split(area);

    render_game_title(frame, chunks[0], app);

    // Split chart area horizontally for chart + players sidebar
    let chart_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(40),    // Chart
            Constraint::Length(22), // Players sidebar
        ])
        .split(chunks[1]);

    render_price_chart(frame, chart_chunks[0], app);
    render_players_sidebar(frame, chart_chunks[1], app);

    render_game_info(frame, chunks[2], app);
    render_game_footer(frame, chunks[3], app);
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
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme::BORDER_INACTIVE)),
        );
    frame.render_widget(title, area);
}

fn render_price_chart(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let (data, x_bounds, y_bounds, price_up, cursor_price, open_orders) = if let Some(ref game) = app.game {
        let x_bounds = game.time_bounds();
        let y_bounds = game.price_bounds();
        let price_up = if game.price_history.len() >= 2 {
            let last = game.price_history.last().map(|(_, p)| *p).unwrap_or(0.0);
            let first = game.price_history.first().map(|(_, p)| *p).unwrap_or(0.0);
            last >= first
        } else {
            true
        };
        (
            game.price_history.clone(),
            x_bounds,
            y_bounds,
            price_up,
            game.cursor_price,
            game.open_orders.clone(),
        )
    } else {
        (vec![(0.0, 100.0)], (0.0, 10.0), (50.0, 150.0), true, 100, vec![])
    };

    // Line color based on price direction
    let line_color = if price_up { theme::GREEN } else { theme::RED };

    // Create cursor line data (horizontal line across full time range)
    let cursor_data: Vec<(f64, f64)> = vec![(x_bounds.0, cursor_price as f64), (x_bounds.1, cursor_price as f64)];

    // Create order line datasets - group orders by type and price
    let mut bid_lines: Vec<Vec<(f64, f64)>> = vec![];
    let mut ask_lines: Vec<Vec<(f64, f64)>> = vec![];
    let mut own_bid_lines: Vec<Vec<(f64, f64)>> = vec![];
    let mut own_ask_lines: Vec<Vec<(f64, f64)>> = vec![];

    for order in &open_orders {
        let line_data = vec![(x_bounds.0, order.price as f64), (x_bounds.1, order.price as f64)];
        match (order.order_type, order.is_own) {
            (OrderType::Bid, true) => own_bid_lines.push(line_data),
            (OrderType::Bid, false) => bid_lines.push(line_data),
            (OrderType::Ask, true) => own_ask_lines.push(line_data),
            (OrderType::Ask, false) => ask_lines.push(line_data),
        }
    }

    let mut datasets = vec![
        Dataset::default()
            .name("PRICE")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(line_color))
            .data(&data),
    ];

    // Add cursor line (amber/yellow)
    datasets.push(
        Dataset::default()
            .name("CURSOR")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme::AMBER))
            .data(&cursor_data),
    );

    // We need to store the data in vectors that live long enough
    // Since we can't store references to local data in the datasets,
    // we need to create the datasets with owned data
    // Ratatui's Chart requires references, so we need to collect all line data first

    // For simplicity, let's create a combined approach
    let all_order_data: Vec<(Vec<(f64, f64)>, bool, OrderType)> = open_orders
        .iter()
        .map(|order| {
            let line_data = vec![(x_bounds.0, order.price as f64), (x_bounds.1, order.price as f64)];
            (line_data, order.is_own, order.order_type)
        })
        .collect();

    // Store references to all the data we'll use in the chart
    let order_datasets: Vec<Dataset> = all_order_data
        .iter()
        .map(|(line_data, is_own, order_type)| {
            let (color, modifier) = match (*order_type, *is_own) {
                (OrderType::Bid, true) => (theme::GREEN, Modifier::BOLD),
                (OrderType::Bid, false) => (theme::GREEN, Modifier::empty()),
                (OrderType::Ask, true) => (theme::RED, Modifier::BOLD),
                (OrderType::Ask, false) => (theme::RED, Modifier::empty()),
            };
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(color).add_modifier(modifier))
                .data(line_data)
        })
        .collect();

    datasets.extend(order_datasets);

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

fn render_players_sidebar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let block = Block::default()
        .title("[ PLAYERS ]")
        .title_style(Style::default().fg(theme::ORANGE).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::BORDER_ACTIVE));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref game) = app.game {
        let starting_price = game.starting_price;

        // Sort players for consistent display
        let mut player_prices: Vec<_> = game.all_prices.iter().collect();
        player_prices.sort_by_key(|(pid, _)| pid.0);

        let items: Vec<ListItem> = player_prices
            .iter()
            .enumerate()
            .map(|(i, (player_id, price))| {
                let player_id = **player_id;
                let price = **price;
                let is_self = app.player_id == Some(player_id);
                let prefix = if is_self { "▶" } else { " " };

                // Price direction from start
                let (arrow, color) = if price > starting_price {
                    ("▲", theme::GREEN)
                } else if price < starting_price {
                    ("▼", theme::RED)
                } else {
                    ("─", Color::White)
                };

                let uuid_str = player_id.0.to_string();
                let short_id = &uuid_str[..6];

                let text = format!("{} P{} {}...", prefix, i + 1, short_id);
                let price_text = format!("${} {}", price, arrow);

                let style = if is_self {
                    Style::default().fg(theme::ORANGE_BRIGHT).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                // Create a two-line item for each player
                ListItem::new(vec![
                    ratatui::text::Line::from(text).style(style),
                    ratatui::text::Line::from(format!("   {}", price_text)).style(Style::default().fg(color)),
                ])
            })
            .collect();

        let list = List::new(items);
        frame.render_widget(list, inner);
    } else {
        let waiting = Paragraph::new("Waiting...")
            .style(Style::default().fg(theme::TEXT_DIM))
            .alignment(Alignment::Center);
        frame.render_widget(waiting, inner);
    }
}

fn render_game_info(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
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

        // Cursor
        let cursor_text = format!("${}", game.cursor_price);
        render_info_box(frame, chunks[1], "CURSOR", &cursor_text, theme::AMBER);

        // Balance
        let balance_text = format!("${}", game.balance);
        render_info_box(frame, chunks[2], "BALANCE", &balance_text, theme::YELLOW_DATA);

        // Shares
        let shares_text = format!("{}", game.shares);
        render_info_box(frame, chunks[3], "SHARES", &shares_text, Color::White);

        // P/L calculation: current value - starting balance
        let current_value = game.balance as i64 + (game.shares as i64 * game.current_price as i64);
        let starting_value = game.starting_balance as i64;
        let pnl = current_value - starting_value;
        let pnl_color = if pnl >= 0 { theme::GREEN } else { theme::RED };
        let pnl_sign = if pnl >= 0 { "+" } else { "" };
        let pnl_text = format!("{}${}", pnl_sign, pnl);
        render_info_box(frame, chunks[4], "P/L", &pnl_text, pnl_color);
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

fn render_game_footer(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let text = if let Some(ref game) = app.game {
        match game.phase {
            GamePhase::Ended => "Q=Return to matchmaking",
            GamePhase::Running => "↑/↓=Move cursor | B=Bid | S=Ask | Q=Quit",
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
