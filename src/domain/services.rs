use std::time::Duration;

use rand::Rng;

use crate::domain::ports::{
    MatchmakingEventNotifier, MatchmakingNotification, MatchmakingQueueRepository, MatchmakingServiceError,
};

use super::ports::{AsyncTimer, GameEventNotifier, GameNotification, GameRepository, GameServiceError};
use super::{GameAction, GameEffect, PlayerId, types::GameId};

pub struct GameService<N, R> {
    notifier: N,
    repository: R,
}

impl<N, R> GameService<N, R>
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

    pub async fn place_bid(
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

    pub async fn place_ask(
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
    pub fn new(
        notifier: N,
        repository: R,
    ) -> Self {
        Self { notifier, repository }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::InMemory;
    use crate::domain::ports::{GameNotification, GameRepository, MatchmakingQueueRepository};
    use crate::domain::{GameEvent, GameState};

    fn create_test_game(
        players: Vec<PlayerId>,
        starting_balance: i32,
    ) -> GameState {
        GameState::init(players, starting_balance)
    }

    // ==================== GameService Tests ====================

    #[tokio::test]
    async fn test_place_bid_success() {
        // Arrange
        let adapter = InMemory::new();
        let game_id = GameId::new();
        let player = PlayerId::new();
        let game = create_test_game(vec![player], 1000);
        adapter.save_game(game_id, &game).await;

        let mut service: GameService<&InMemory, &InMemory> = GameService::new(&adapter, &adapter);

        // Act
        let result = service.place_bid(game_id, player, 50).await;

        // Assert
        assert!(result.is_ok());
        let events = adapter.get_game_events();
        assert!(!events.is_empty(), "Expected notifications to be sent");

        // Verify the bid was placed - should notify the player
        let has_bid_placed = events.iter().any(|(pid, notif)| {
            *pid == player
                && matches!(
                    notif,
                    GameNotification::GameEvent(GameEvent::BidPlaced { player_id, bid_value })
                    if *player_id == player && *bid_value == 50
                )
        });
        assert!(has_bid_placed, "Expected BidPlaced notification");
    }

    #[tokio::test]
    async fn test_place_bid_game_not_found() {
        // Arrange
        let adapter = InMemory::new();
        let game_id = GameId::new();
        let player = PlayerId::new();
        // Note: NOT saving any game to the repository

        let mut service: GameService<&InMemory, &InMemory> = GameService::new(&adapter, &adapter);

        // Act
        let result = service.place_bid(game_id, player, 50).await;

        // Assert
        assert!(matches!(result, Err(GameServiceError::GameNotFound(id)) if id == game_id));
    }

    #[tokio::test]
    async fn test_place_ask_success() {
        // Arrange
        let adapter = InMemory::new();
        let game_id = GameId::new();
        let player = PlayerId::new();
        let mut game = create_test_game(vec![player], 1000);
        // Give player some shares first by processing a price and resolved bid
        game.process_action(GameAction::Bid {
            player_id: player,
            bid_value: 50,
        });
        game.process_action(GameAction::SetPrice(50)); // This resolves the bid, giving shares
        adapter.save_game(game_id, &game).await;

        let mut service: GameService<&InMemory, &InMemory> = GameService::new(&adapter, &adapter);

        // Act
        let result = service.place_ask(game_id, player, 60).await;

        // Assert
        assert!(result.is_ok());
        let events = adapter.get_game_events();
        assert!(!events.is_empty(), "Expected notifications to be sent");
    }

    #[tokio::test]
    async fn test_place_ask_game_not_found() {
        // Arrange
        let adapter = InMemory::new();
        let game_id = GameId::new();
        let player = PlayerId::new();
        // Note: NOT saving any game to the repository

        let mut service: GameService<&InMemory, &InMemory> = GameService::new(&adapter, &adapter);

        // Act
        let result = service.place_ask(game_id, player, 50).await;

        // Assert
        assert!(matches!(result, Err(GameServiceError::GameNotFound(id)) if id == game_id));
    }

    // ==================== MatchmakingService Tests ====================

    #[tokio::test]
    async fn test_join_queue_adds_player() {
        // Arrange
        let adapter = InMemory::new();
        let player = PlayerId::new();

        let service: MatchmakingService<&InMemory, &InMemory> = MatchmakingService::new(&adapter, &adapter);

        // Act
        let result = service.join_queue(player).await;

        // Assert
        assert!(result.is_ok());
        let queue: Vec<PlayerId> = adapter.load_queue().await;
        assert!(queue.contains(&player), "Player should be in queue");

        let events = adapter.get_matchmaking_events();
        let has_joined_notification = events.iter().any(|(pid, notif)| {
            *pid == player && matches!(notif, MatchmakingNotification::PlayerJoinedQueue(joined) if *joined == player)
        });
        assert!(has_joined_notification, "Expected PlayerJoinedQueue notification");
    }

    #[tokio::test]
    async fn test_leave_queue_removes_player() {
        // Arrange
        let adapter = InMemory::new();
        let player1 = PlayerId::new();
        let player2 = PlayerId::new();

        let service: MatchmakingService<&InMemory, &InMemory> = MatchmakingService::new(&adapter, &adapter);
        let _ = service.join_queue(player1).await;
        let _ = service.join_queue(player2).await;

        // Act
        let result = service.leave_queue(player1).await;

        // Assert
        assert!(result.is_ok());
        let queue: Vec<PlayerId> = adapter.load_queue().await;
        assert!(!queue.contains(&player1), "Player1 should not be in queue");
        assert!(queue.contains(&player2), "Player2 should still be in queue");

        let events = adapter.get_matchmaking_events();
        let has_left_notification = events
            .iter()
            .any(|(_, notif)| matches!(notif, MatchmakingNotification::PlayerLeftQueue(left) if *left == player1));
        assert!(has_left_notification, "Expected PlayerLeftQueue notification");
    }

    #[tokio::test]
    async fn test_game_found_notifies_matched_players() {
        // Arrange
        let adapter = InMemory::new();
        let player1 = PlayerId::new();
        let player2 = PlayerId::new();
        let game_id = GameId::new();

        let service: MatchmakingService<&InMemory, &InMemory> = MatchmakingService::new(&adapter, &adapter);
        let _ = service.join_queue(player1).await;
        let _ = service.join_queue(player2).await;

        // Act
        let result = service.game_found(vec![player1, player2], game_id).await;

        // Assert
        assert!(result.is_ok());

        let events = adapter.get_matchmaking_events();
        let player1_got_game_found = events.iter().any(|(pid, notif)| {
            *pid == player1 && matches!(notif, MatchmakingNotification::GameFound(gid) if *gid == game_id)
        });
        let player2_got_game_found = events.iter().any(|(pid, notif)| {
            *pid == player2 && matches!(notif, MatchmakingNotification::GameFound(gid) if *gid == game_id)
        });
        assert!(player1_got_game_found, "Player1 should receive GameFound notification");
        assert!(player2_got_game_found, "Player2 should receive GameFound notification");
    }

    // ==================== PriceTickerHandler Tests ====================

    #[tokio::test]
    async fn test_next_price_stays_non_negative() {
        // Arrange
        let adapter = InMemory::new();
        let handler: PriceTickerHandler<&InMemory, &InMemory, &InMemory> = PriceTickerHandler::new(
            &adapter,
            &adapter,
            &adapter,
            Duration::from_millis(10),
            100, // max_price_delta
        );

        // Act & Assert - run many times to test randomness
        for _ in 0..100 {
            let next = handler.next_price(0);
            assert!(next >= 0, "Price should never be negative, got {}", next);
        }
    }

    #[tokio::test]
    async fn test_next_price_within_delta_range() {
        // Arrange
        let adapter = InMemory::new();
        let max_delta = 10;
        let handler: PriceTickerHandler<&InMemory, &InMemory, &InMemory> =
            PriceTickerHandler::new(&adapter, &adapter, &adapter, Duration::from_millis(10), max_delta);

        // Act & Assert
        let current_price = 100;
        for _ in 0..100 {
            let next = handler.next_price(current_price);
            let delta = (next - current_price).abs();
            assert!(delta <= max_delta, "Price delta {} exceeds max_delta {}", delta, max_delta);
        }
    }
}
