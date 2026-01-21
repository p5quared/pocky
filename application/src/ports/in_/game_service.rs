use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::ports::out_::{GameEventNotifier, GameNotification, GameServiceError};
use domain::{GameAction, GameConfig, GameEffect, GameEvent, GameId, GameState, PlayerId};

pub type GameStore = Arc<RwLock<HashMap<GameId, GameState>>>;

pub enum GameUseCase {
    PlaceBid {
        game_id: GameId,
        player_id: PlayerId,
        value: i32,
    },
    PlaceAsk {
        game_id: GameId,
        player_id: PlayerId,
        value: i32,
    },
    CancelBid {
        game_id: GameId,
        player_id: PlayerId,
        price: i32,
    },
    CancelAsk {
        game_id: GameId,
        player_id: PlayerId,
        price: i32,
    },
    LaunchGame {
        players: Vec<PlayerId>,
        config: GameConfig,
    },
}

pub async fn execute<N: GameEventNotifier + 'static>(
    notifier: Arc<N>,
    game_store: GameStore,
    use_case: GameUseCase,
) -> Result<(), GameServiceError> {
    match use_case {
        GameUseCase::PlaceBid {
            game_id,
            player_id,
            value,
        } => {
            process_action(
                notifier,
                game_store,
                game_id,
                GameAction::Bid {
                    player_id,
                    bid_value: value,
                },
            )
            .await
        }
        GameUseCase::PlaceAsk {
            game_id,
            player_id,
            value,
        } => {
            process_action(
                notifier,
                game_store,
                game_id,
                GameAction::Ask {
                    player_id,
                    ask_value: value,
                },
            )
            .await
        }
        GameUseCase::CancelBid {
            game_id,
            player_id,
            price,
        } => {
            process_action(
                notifier,
                game_store,
                game_id,
                GameAction::CancelBid { player_id, price },
            )
            .await
        }
        GameUseCase::CancelAsk {
            game_id,
            player_id,
            price,
        } => {
            process_action(
                notifier,
                game_store,
                game_id,
                GameAction::CancelAsk { player_id, price },
            )
            .await
        }
        GameUseCase::LaunchGame { players, config } => {
            let game_id = GameId::new();
            let (game_state, effects) = GameState::launch(players, config);

            game_store.write().await.insert(game_id, game_state);
            process_effects(notifier, game_store, game_id, effects);
            Ok(())
        }
    }
}

async fn process_action<N: GameEventNotifier + 'static>(
    notifier: Arc<N>,
    game_store: GameStore,
    game_id: GameId,
    action: GameAction,
) -> Result<(), GameServiceError> {
    let effects = {
        let mut store = game_store.write().await;
        let Some(game_state) = store.get_mut(&game_id) else {
            return Err(GameServiceError::GameNotFound(game_id));
        };
        game_state.process_action(action)?
    };

    process_effects(notifier, game_store, game_id, effects);
    Ok(())
}

fn process_effects<N: GameEventNotifier + 'static>(
    notifier: Arc<N>,
    game_store: GameStore,
    game_id: GameId,
    effects: Vec<GameEffect>,
) {
    for effect in effects {
        match effect {
            GameEffect::Notify { player_id, event } => {
                let notification = match event {
                    GameEvent::Countdown(remaining) => GameNotification::Countdown { game_id, remaining },
                    GameEvent::GameStarted {
                        starting_price,
                        starting_balance,
                        players,
                    } => GameNotification::GameStarted {
                        game_id,
                        starting_price,
                        starting_balance,
                        players,
                    },
                    GameEvent::PriceChanged(price) => GameNotification::PriceChanged { game_id, price },
                    GameEvent::BidPlaced { player_id, bid_value } => GameNotification::BidPlaced {
                        game_id,
                        player_id,
                        bid_value,
                    },
                    GameEvent::AskPlaced { player_id, ask_value } => GameNotification::AskPlaced {
                        game_id,
                        player_id,
                        ask_value,
                    },
                    GameEvent::BidFilled { player_id, bid_value } => GameNotification::BidFilled {
                        game_id,
                        player_id,
                        bid_value,
                    },
                    GameEvent::AskFilled { player_id, ask_value } => GameNotification::AskFilled {
                        game_id,
                        player_id,
                        ask_value,
                    },
                    GameEvent::BidCanceled { player_id, price } => GameNotification::BidCanceled {
                        game_id,
                        player_id,
                        price,
                    },
                    GameEvent::AskCanceled { player_id, price } => GameNotification::AskCanceled {
                        game_id,
                        player_id,
                        price,
                    },
                    GameEvent::GameEnded { final_balances } => GameNotification::GameEnded {
                        game_id,
                        final_balances,
                    },
                };
                let notifier = Arc::clone(&notifier);
                tokio::spawn(async move {
                    notifier.notify_player(player_id, notification).await;
                });
            }
            GameEffect::DelayedAction { delay, action } => {
                let notifier = Arc::clone(&notifier);
                let game_store = Arc::clone(&game_store);
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    let _ = process_action(notifier, game_store, game_id, action).await;
                });
            }
        }
    }
}
