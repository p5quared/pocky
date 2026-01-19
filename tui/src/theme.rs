//! Bloomberg Terminal-inspired theme colors and styles

use ratatui::style::Color;

// Bloomberg Orange (primary accent)
pub const ORANGE: Color = Color::Rgb(255, 136, 0); // #FF8800
pub const ORANGE_BRIGHT: Color = Color::Rgb(255, 170, 51); // #FFAA33

// Status colors
pub const GREEN: Color = Color::Rgb(0, 204, 102); // #00CC66 - positive/connected
pub const RED: Color = Color::Rgb(255, 51, 51); // #FF3333 - negative/error
pub const AMBER: Color = Color::Rgb(255, 191, 0); // #FFBF00 - warning/connecting

// Data display
pub const YELLOW_DATA: Color = Color::Rgb(255, 255, 102); // #FFFF66 - values

// Text
pub const TEXT_DIM: Color = Color::Rgb(102, 102, 102); // #666666
pub const TEXT_SECONDARY: Color = Color::Rgb(170, 170, 170); // #AAAAAA

// Borders
pub const BORDER_ACTIVE: Color = Color::Rgb(255, 136, 0); // Orange
pub const BORDER_INACTIVE: Color = Color::Rgb(68, 68, 68); // #444444
