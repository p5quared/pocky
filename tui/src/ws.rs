use domain::{GameId, PlayerId};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::events::AppEvent;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutgoingMessage {
    JoinQueue,
    LeaveQueue,
    PlaceBid { game_id: GameId, value: i32 },
    PlaceAsk { game_id: GameId, value: i32 },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameNotification {
    Countdown {
        game_id: GameId,
        remaining: u32,
    },
    GameStarted {
        game_id: GameId,
        starting_price: i32,
        starting_balance: i32,
        players: Vec<PlayerId>,
    },
    PriceChanged {
        game_id: GameId,
        player_id: PlayerId,
        price: i32,
    },
    BidPlaced {
        game_id: GameId,
        player_id: PlayerId,
        bid_value: i32,
    },
    AskPlaced {
        game_id: GameId,
        player_id: PlayerId,
        ask_value: i32,
    },
    BidFilled {
        game_id: GameId,
        player_id: PlayerId,
        bid_value: i32,
    },
    AskFilled {
        game_id: GameId,
        player_id: PlayerId,
        ask_value: i32,
    },
    GameEnded {
        game_id: GameId,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub enum MatchmakingMessage {
    Matched(Vec<PlayerId>),
    Enqueued(PlayerId),
    Dequeued(PlayerId),
    PlayerNotFound,
    AlreadyQueued,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ServerMessage {
    Game(GameNotification),
    Matchmaking(MatchmakingMessage),
}

pub enum WsCommand {
    Connect,
    Send(OutgoingMessage),
}

pub async fn websocket_loop(
    url: &str,
    mut cmd_rx: mpsc::Receiver<WsCommand>,
    event_tx: mpsc::Sender<AppEvent>,
) {
    loop {
        match cmd_rx.recv().await {
            Some(WsCommand::Connect) => {}
            Some(WsCommand::Send(_)) => continue,
            None => return,
        }

        let ws_stream = match connect_async(url).await {
            Ok((stream, _)) => {
                let _ = event_tx.send(AppEvent::WsConnected).await;
                stream
            }
            Err(e) => {
                let _ = event_tx.send(AppEvent::WsError(e.to_string())).await;
                continue;
            }
        };

        let (mut write, mut read) = ws_stream.split();

        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(WsCommand::Send(msg)) => {
                            let json = serde_json::to_string(&msg).unwrap();
                            if write.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                        Some(WsCommand::Connect) => {
                            // Already connected
                        }
                        None => {
                            let _ = write.close().await;
                            return;
                        }
                    }
                }

                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<ServerMessage>(&text) {
                                Ok(server_msg) => {
                                    let _ = event_tx.send(AppEvent::WsMessage(server_msg)).await;
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse message: {}", e);
                                    eprintln!("Raw message: {}", text);
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            let _ = event_tx.send(AppEvent::WsDisconnected).await;
                            break;
                        }
                        Some(Err(e)) => {
                            let _ = event_tx.send(AppEvent::WsError(e.to_string())).await;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
