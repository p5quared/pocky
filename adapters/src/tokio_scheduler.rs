use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use domain::{GameAction, GameEffect, GameId};
use application::ports::out_::{GameEventNotifier, GameEventScheduler, GameNotification, GameRepository, GameServiceError};

/// Type aliases for the dynamic trait objects
pub type DynNotifier = Arc<dyn GameEventNotifier>;
pub type DynRepository = Arc<dyn GameRepository>;

/// Process a game action: load state, process, save, notify.
/// Returns the effects for caller to handle (including DelayedAction).
pub async fn process_game_action(
    notifier: &dyn GameEventNotifier,
    repository: &dyn GameRepository,
    game_id: GameId,
    action: GameAction,
) -> Result<Vec<GameEffect>, GameServiceError> {
    let Some(mut game_state) = repository.load_game(game_id).await else {
        return Err(GameServiceError::GameNotFound(game_id));
    };

    let effects = game_state.process_action(action)?;
    repository.save_game(game_id, &game_state).await;

    for effect in &effects {
        if let GameEffect::Notify { player_id, event } = effect {
            notifier.notify_player(*player_id, GameNotification::GameEvent(*event)).await;
        }
    }

    Ok(effects)
}

/// Execute a scheduled action and spawn tasks for any resulting DelayedAction effects.
fn execute_and_reschedule(
    notifier: DynNotifier,
    repository: DynRepository,
    game_id: GameId,
    action: GameAction,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
        let result = process_game_action(notifier.as_ref(), repository.as_ref(), game_id, action).await;

        if let Ok(effects) = result {
            for effect in effects {
                if let GameEffect::DelayedAction { delay_ms, action } = effect {
                    let notifier = Arc::clone(&notifier);
                    let repository = Arc::clone(&repository);
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        execute_and_reschedule(notifier, repository, game_id, action).await;
                    });
                }
            }
        }
    })
}

pub struct TokioGameScheduler {
    notifier: DynNotifier,
    repository: DynRepository,
}

impl TokioGameScheduler {
    pub fn new(
        notifier: DynNotifier,
        repository: DynRepository,
    ) -> Self {
        Self { notifier, repository }
    }
}

#[async_trait]
impl GameEventScheduler for TokioGameScheduler {
    async fn schedule_action(
        &self,
        game_id: GameId,
        delay: Duration,
        action: GameAction,
    ) {
        let notifier = Arc::clone(&self.notifier);
        let repository = Arc::clone(&self.repository);
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            execute_and_reschedule(notifier, repository, game_id, action).await;
        });
    }
}
