use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::ports::out_::{GameEventNotifier, GameNotification, GameServiceError};
use domain::{GameAction, GameConfig, GameEffect, GameId, GameState, PlayerId};

pub type GameStore = Arc<RwLock<HashMap<GameId, GameState>>>;

pub async fn place_bid<N: GameEventNotifier + 'static>(
    notifier: Arc<N>,
    game_store: GameStore,
    game_id: GameId,
    player_id: PlayerId,
    bid_value: i32,
) -> Result<(), GameServiceError> {
    process_action(notifier, game_store, game_id, GameAction::Bid { player_id, bid_value }).await
}

pub async fn place_ask<N: GameEventNotifier + 'static>(
    notifier: Arc<N>,
    game_store: GameStore,
    game_id: GameId,
    player_id: PlayerId,
    ask_value: i32,
) -> Result<(), GameServiceError> {
    process_action(notifier, game_store, game_id, GameAction::Ask { player_id, ask_value }).await
}

pub async fn launch_game<N: GameEventNotifier + 'static>(
    notifier: Arc<N>,
    game_store: GameStore,
    players: Vec<PlayerId>,
    config: GameConfig,
) -> Result<GameId, GameServiceError> {
    let game_id = GameId::new();
    let (game_state, effects) = GameState::launch(players, config);

    game_store.write().await.insert(game_id, game_state);
    process_effects(notifier, game_store, game_id, effects);

    Ok(game_id)
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
                let notifier = Arc::clone(&notifier);
                tokio::spawn(async move {
                    notifier.notify_player(player_id, GameNotification::GameEvent(event)).await;
                });
            }
            GameEffect::DelayedAction { delay_ms, action } => {
                let notifier = Arc::clone(&notifier);
                let game_store = Arc::clone(&game_store);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    let _ = process_action(notifier, game_store, game_id, action).await;
                });
            }
        }
    }
}
