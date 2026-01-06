use std::time::Duration;

use domain::{GameAction, GameConfig, GameEffect, GameId, GameState, PlayerId};
use crate::ports::out_::{
    GameEventNotifier, GameEventScheduler, GameNotification, GameRepository, GameServiceError,
};

pub struct GameService<N, R, S> {
    notifier: N,
    repository: R,
    scheduler: S,
}

impl<N, R, S> GameService<N, R, S>
where
    N: GameEventNotifier,
    R: GameRepository,
    S: GameEventScheduler,
{
    pub fn new(
        notifier: N,
        repository: R,
        scheduler: S,
    ) -> Self {
        Self {
            notifier,
            repository,
            scheduler,
        }
    }

    pub async fn place_bid(
        &mut self,
        game_id: GameId,
        player_id: PlayerId,
        bid_value: i32,
    ) -> Result<(), GameServiceError> {
        let Some(mut game_state) = self.repository.load_game(game_id).await else {
            return Err(GameServiceError::GameNotFound(game_id));
        };

        let effects = game_state.process_action(GameAction::Bid { player_id, bid_value })?;
        self.process_effects(game_id, effects).await;

        self.repository.save_game(game_id, &game_state).await;
        Ok(())
    }

    pub async fn place_ask(
        &mut self,
        game_id: GameId,
        player_id: PlayerId,
        ask_value: i32,
    ) -> Result<(), GameServiceError> {
        let Some(mut game_state) = self.repository.load_game(game_id).await else {
            return Err(GameServiceError::GameNotFound(game_id));
        };

        let effects = game_state.process_action(GameAction::Ask { player_id, ask_value })?;
        self.process_effects(game_id, effects).await;

        self.repository.save_game(game_id, &game_state).await;
        Ok(())
    }

    pub async fn launch_game(
        &mut self,
        players: Vec<PlayerId>,
        starting_balance: i32,
        config: GameConfig,
    ) -> Result<GameId, GameServiceError> {
        let game_id = GameId::new();
        let (game_state, effects) = GameState::launch(players, starting_balance, config);

        self.repository.save_game(game_id, &game_state).await;
        self.process_effects(game_id, effects).await;

        Ok(game_id)
    }

    pub async fn process_price_tick(
        &mut self,
        game_id: GameId,
    ) -> Result<(), GameServiceError> {
        let Some(mut game_state) = self.repository.load_game(game_id).await else {
            return Err(GameServiceError::GameNotFound(game_id));
        };

        let effects = game_state.process_action(GameAction::Tick)?;
        self.repository.save_game(game_id, &game_state).await;

        self.process_effects(game_id, effects).await;

        Ok(())
    }

    async fn process_effects(
        &mut self,
        game_id: GameId,
        effects: Vec<GameEffect>,
    ) {
        for effect in effects {
            match effect {
                GameEffect::Notify { player_id, event } => {
                    self.notifier
                        .notify_player(player_id, GameNotification::GameEvent(event))
                        .await;
                }
                GameEffect::DelayedAction { delay_ms, action } => {
                    self.scheduler
                        .schedule_action(game_id, Duration::from_millis(delay_ms), action)
                        .await;
                }
            }
        }
    }
}
