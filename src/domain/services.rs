use std::time::Duration;

use rand::Rng;

use crate::domain::ports::{
    MatchmakingEventNotifier, MatchmakingNotification, MatchmakingQueueRepository, MatchmakingServiceError,
};

use super::ports::{AsyncTimer, GameEventNotifier, GameNotification, GameRepository, GameServiceError};
use super::{GameAction, GameEffect, PlayerId, types::GameId};

pub struct PlaceBidHandler<N, R> {
    notifier: N,
    repository: R,
}

impl<N, R> PlaceBidHandler<N, R>
where
    N: GameEventNotifier,
    R: GameRepository,
{
    pub fn new(
        notifier: N,
        repository: R,
    ) -> Self {
        Self { notifier, repository }
    }

    pub async fn execute(
        &mut self,
        game_id: GameId,
        player_id: PlayerId,
        bid_value: i32,
    ) -> Result<(), GameServiceError> {
        let Some(mut game_state) = self.repository.load_game(game_id).await else {
            return Err(GameServiceError::GameNotFound(game_id));
        };

        let effects = game_state.process_action(GameAction::Bid { player_id, bid_value });
        self.process_effects(effects).await;

        self.repository.save_game(game_id, &game_state).await;
        Ok(())
    }

    async fn process_effects(
        &mut self,
        effects: Vec<GameEffect>,
    ) {
        for effect in effects {
            match effect {
                GameEffect::Notify { player_id, event } => {
                    self.notifier
                        .notify_player(player_id, GameNotification::GameEvent(event))
                        .await;
                }
            }
        }
    }
}

pub struct PlaceAskHandler<N, R> {
    notifier: N,
    repository: R,
}

impl<N, R> PlaceAskHandler<N, R>
where
    N: GameEventNotifier,
    R: GameRepository,
{
    pub fn new(
        notifier: N,
        repository: R,
    ) -> Self {
        Self { notifier, repository }
    }

    pub async fn execute(
        &mut self,
        game_id: GameId,
        player_id: PlayerId,
        ask_value: i32,
    ) -> Result<(), GameServiceError> {
        let Some(mut game_state) = self.repository.load_game(game_id).await else {
            return Err(GameServiceError::GameNotFound(game_id));
        };

        let effects = game_state.process_action(GameAction::Ask { player_id, ask_value });
        self.process_effects(effects).await;

        self.repository.save_game(game_id, &game_state).await;
        Ok(())
    }

    async fn process_effects(
        &mut self,
        effects: Vec<GameEffect>,
    ) {
        for effect in effects {
            match effect {
                GameEffect::Notify { player_id, event } => {
                    self.notifier
                        .notify_player(player_id, GameNotification::GameEvent(event))
                        .await;
                }
            }
        }
    }
}

pub struct PriceTickerHandler<N, R, T> {
    notifier: N,
    repository: R,
    timer: T,
    tick_interval: Duration,
    max_price_delta: i32,
}

impl<N, R, T> PriceTickerHandler<N, R, T>
where
    N: GameEventNotifier,
    R: GameRepository,
    T: AsyncTimer,
{
    pub fn new(
        notifier: N,
        repository: R,
        timer: T,
        tick_interval: Duration,
        max_price_delta: i32,
    ) -> Self {
        Self {
            notifier,
            repository,
            timer,
            tick_interval,
            max_price_delta,
        }
    }

    pub async fn run(
        &mut self,
        game_id: GameId,
        initial_price: i32,
    ) -> Result<(), GameServiceError> {
        let mut current_price = initial_price;

        loop {
            let Some(mut game_state) = self.repository.load_game(game_id).await else {
                return Err(GameServiceError::GameNotFound(game_id));
            };

            let effects = game_state.process_action(GameAction::SetPrice(current_price));
            self.process_effects(effects).await;

            self.repository.save_game(game_id, &game_state).await;

            self.timer.sleep(self.tick_interval).await;

            current_price = self.next_price(current_price);
        }
    }

    fn next_price(
        &self,
        current_price: i32,
    ) -> i32 {
        let mut rng = rand::thread_rng();
        let delta = rng.gen_range(-self.max_price_delta..=self.max_price_delta);
        (current_price + delta).max(0)
    }

    async fn process_effects(
        &mut self,
        effects: Vec<GameEffect>,
    ) {
        for effect in effects {
            match effect {
                GameEffect::Notify { player_id, event } => {
                    self.notifier
                        .notify_player(player_id, GameNotification::GameEvent(event))
                        .await;
                }
            }
        }
    }
}

// NOTE: At some point we may need to create a domain for this
// as we develop a more intelligent matchmaking system
pub struct MatchmakingService<N, R> {
    notifier: N,
    repository: R,
}

impl<N, R> MatchmakingService<N, R>
where
    N: MatchmakingEventNotifier,
    R: MatchmakingQueueRepository,
{
    pub async fn join_queue(
        &self,
        player_id: PlayerId,
    ) -> Result<(), MatchmakingServiceError> {
        let mut queue = self.repository.load_queue().await;
        queue.push(player_id);
        self.repository.save_queue(&queue).await;

        for queued_player in queue {
            self.notifier
                .notify_player(queued_player, MatchmakingNotification::PlayerJoinedQueue(player_id))
                .await;
        }

        Ok(())
    }

    pub async fn leave_queue(
        &self,
        player_id: PlayerId,
    ) -> Result<(), MatchmakingServiceError> {
        let queue = self.repository.load_queue().await;
        let queue_without_player: Vec<PlayerId> = queue.into_iter().filter(|p| *p != player_id).collect();
        self.repository.save_queue(&queue_without_player).await;

        for queued_player in queue_without_player {
            self.notifier
                .notify_player(queued_player, MatchmakingNotification::PlayerLeftQueue(player_id))
                .await;
        }

        Ok(())
    }

    pub async fn game_found(
        &self,
        matched_players: Vec<PlayerId>,
        game_id: GameId,
    ) -> Result<(), MatchmakingServiceError> {
        let queue = self.repository.load_queue().await;
        let queue_without_players: Vec<PlayerId> = queue.into_iter().filter(|p| matched_players.contains(p)).collect();
        self.repository.save_queue(&queue_without_players).await;

        for queued_player in queue_without_players {
            for player_id in &matched_players {
                self.notifier
                    .notify_player(queued_player, MatchmakingNotification::PlayerLeftQueue(*player_id))
                    .await;
            }
        }

        for player_id in matched_players {
            self.notifier
                .notify_player(player_id, MatchmakingNotification::GameFound(game_id))
                .await;
        }

        Ok(())
    }
}
