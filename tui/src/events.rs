use crate::ws::ServerMessage;
use crossterm::event::KeyEvent;

pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    WsConnected,
    WsDisconnected,
    WsError(String),
    WsMessage(ServerMessage),
}
