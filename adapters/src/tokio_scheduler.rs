use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use domain::{GameAction, GameEffect, GameId};
use application::ports::out_::{GameEventNotifier, GameEventScheduler, GameNotification, GameRepository, GameServiceError};

/// Process a game action: load state, process, save, notify.
/// Returns the effects for caller to handle (including DelayedAction).
pub async fn process_game_action<N, R>(
    notifier: &N,
    repository: &R,
    game_id: GameId,
    action: GameAction,
) -> Result<Vec<GameEffect>, GameServiceError>
where
    N: GameEventNotifier,
    R: GameRepository,
{
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
fn execute_and_reschedule<N, R>(
    notifier: Arc<N>,
    repository: Arc<R>,
    game_id: GameId,
    action: GameAction,
) -> Pin<Box<dyn Future<Output = ()> + Send>>
where
    N: GameEventNotifier + Send + Sync + 'static,
    R: GameRepository + Send + Sync + 'static,
{
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

pub struct TokioGameScheduler<N, R> {
    notifier: Arc<N>,
    repository: Arc<R>,
}

impl<N, R> TokioGameScheduler<N, R>
where
    N: GameEventNotifier + Send + Sync + 'static,
    R: GameRepository + Send + Sync + 'static,
{
    pub fn new(
        notifier: Arc<N>,
        repository: Arc<R>,
    ) -> Self {
        Self { notifier, repository }
    }
}

impl<N, R> GameEventScheduler for TokioGameScheduler<N, R>
where
    N: GameEventNotifier + Send + Sync + 'static,
    R: GameRepository + Send + Sync + 'static,
{
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

impl<N, R> GameEventScheduler for &TokioGameScheduler<N, R>
where
    N: GameEventNotifier + Send + Sync + 'static,
    R: GameRepository + Send + Sync + 'static,
{
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
