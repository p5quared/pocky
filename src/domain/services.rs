use std::time::Duration;

use crate::domain::ports::{
    LobbyEventNotifier, LobbyNotification, LobbyPlayerInfo, LobbyRepository, LobbyServiceError, MatchmakingEventNotifier,
    MatchmakingNotification, MatchmakingQueueRepository, MatchmakingServiceError,
};

use super::ports::{AsyncTimer, GameEventNotifier, GameNotification, GameRepository, GameServiceError};
use super::types::LobbyId;
use super::{GameAction, GameConfig, GameEffect, LobbyAction, LobbyEffect, LobbyPhase, LobbyState, PlayerId, types::GameId};

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

        let effects = game_state.process_action(GameAction::Bid { player_id, bid_value })?;
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

        let effects = game_state.process_action(GameAction::Ask { player_id, ask_value })?;
        self.process_effects(effects).await;

        self.repository.save_game(game_id, &game_state).await;
        Ok(())
    }

    pub async fn new_game(
        &mut self,
        players: Vec<PlayerId>,
        starting_balance: i32,
        config: GameConfig,
    ) -> Result<GameId, GameServiceError> {
        let game_id = GameId::new();
        let game_state = super::GameState::new(players, starting_balance, config);
        self.repository.save_game(game_id, &game_state).await;
        Ok(game_id)
    }

    pub async fn start_game(
        &mut self,
        game_id: GameId,
    ) -> Result<Option<u64>, GameServiceError> {
        let Some(mut game_state) = self.repository.load_game(game_id).await else {
            return Err(GameServiceError::GameNotFound(game_id));
        };

        let effects = game_state.process_action(GameAction::Start)?;
        self.repository.save_game(game_id, &game_state).await;

        let scheduled_delay = Self::extract_scheduled_tick(&effects);
        self.process_effects(effects).await;

        Ok(scheduled_delay)
    }

    pub async fn process_price_tick(
        &mut self,
        game_id: GameId,
    ) -> Result<Option<u64>, GameServiceError> {
        let Some(mut game_state) = self.repository.load_game(game_id).await else {
            return Err(GameServiceError::GameNotFound(game_id));
        };

        let effects = game_state.process_action(GameAction::PriceTick)?;
        self.repository.save_game(game_id, &game_state).await;

        let scheduled_delay = Self::extract_scheduled_tick(&effects);
        self.process_effects(effects).await;

        Ok(scheduled_delay)
    }

    pub fn extract_scheduled_tick(effects: &[GameEffect]) -> Option<u64> {
        effects.iter().find_map(|e| match e {
            GameEffect::SchedulePriceTick { delay_ms } => Some(*delay_ms),
            _ => None,
        })
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
                GameEffect::SchedulePriceTick { .. } => {
                    // Handled by caller via extract_scheduled_tick
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
        lobby_id: LobbyId,
    ) -> Result<(), MatchmakingServiceError> {
        let queue = self.repository.load_queue().await;
        // Fix: filter to EXCLUDE matched players (was incorrectly keeping them)
        let queue_without_players: Vec<PlayerId> = queue.into_iter().filter(|p| !matched_players.contains(p)).collect();
        self.repository.save_queue(&queue_without_players).await;

        // Notify remaining queue members that matched players left
        for queued_player in &queue_without_players {
            for player_id in &matched_players {
                self.notifier
                    .notify_player(*queued_player, MatchmakingNotification::PlayerLeftQueue(*player_id))
                    .await;
            }
        }

        // Notify matched players about the lobby
        for player_id in matched_players {
            self.notifier
                .notify_player(player_id, MatchmakingNotification::LobbyCreated(lobby_id))
                .await;
        }

        Ok(())
    }
}

pub struct LobbyService<N, R> {
    notifier: N,
    repository: R,
}

impl<N, R> LobbyService<N, R>
where
    N: LobbyEventNotifier,
    R: LobbyRepository,
{
    pub fn new(
        notifier: N,
        repository: R,
    ) -> Self {
        Self { notifier, repository }
    }

    pub async fn create_lobby(
        &self,
        players: Vec<PlayerId>,
    ) -> Result<LobbyId, LobbyServiceError> {
        let lobby_id = LobbyId::new();
        let lobby = LobbyState::new(lobby_id, players);
        self.repository.save_lobby(lobby_id, &lobby).await;
        Ok(lobby_id)
    }

    pub async fn player_arrived(
        &self,
        lobby_id: LobbyId,
        player_id: PlayerId,
    ) -> Result<(), LobbyServiceError> {
        let Some(mut lobby) = self.repository.load_lobby(lobby_id).await else {
            return Err(LobbyServiceError::LobbyNotFound(lobby_id));
        };

        let effects = lobby.process_action(LobbyAction::PlayerArrived(player_id));
        self.repository.save_lobby(lobby_id, &lobby).await;
        self.process_effects(&lobby, effects).await;

        // Send current lobby state to the arriving player
        self.send_lobby_state(&lobby, player_id).await;

        Ok(())
    }

    pub async fn player_ready(
        &self,
        lobby_id: LobbyId,
        player_id: PlayerId,
    ) -> Result<(), LobbyServiceError> {
        let Some(mut lobby) = self.repository.load_lobby(lobby_id).await else {
            return Err(LobbyServiceError::LobbyNotFound(lobby_id));
        };

        let effects = lobby.process_action(LobbyAction::PlayerReady(player_id));
        self.repository.save_lobby(lobby_id, &lobby).await;
        self.process_effects(&lobby, effects).await;

        Ok(())
    }

    pub async fn player_unready(
        &self,
        lobby_id: LobbyId,
        player_id: PlayerId,
    ) -> Result<(), LobbyServiceError> {
        let Some(mut lobby) = self.repository.load_lobby(lobby_id).await else {
            return Err(LobbyServiceError::LobbyNotFound(lobby_id));
        };

        let effects = lobby.process_action(LobbyAction::PlayerUnready(player_id));
        self.repository.save_lobby(lobby_id, &lobby).await;
        self.process_effects(&lobby, effects).await;

        Ok(())
    }

    pub async fn player_disconnected(
        &self,
        lobby_id: LobbyId,
        player_id: PlayerId,
    ) -> Result<(), LobbyServiceError> {
        let Some(mut lobby) = self.repository.load_lobby(lobby_id).await else {
            return Err(LobbyServiceError::LobbyNotFound(lobby_id));
        };

        let effects = lobby.process_action(LobbyAction::PlayerDisconnected(player_id));
        self.repository.save_lobby(lobby_id, &lobby).await;
        self.process_effects(&lobby, effects).await;

        Ok(())
    }

    pub async fn countdown_tick(
        &self,
        lobby_id: LobbyId,
    ) -> Result<Option<GameId>, LobbyServiceError> {
        let Some(mut lobby) = self.repository.load_lobby(lobby_id).await else {
            return Err(LobbyServiceError::LobbyNotFound(lobby_id));
        };

        let effects = lobby.process_action(LobbyAction::CountdownTick);
        self.repository.save_lobby(lobby_id, &lobby).await;

        // Check if game creation was triggered
        let game_id = effects.iter().find_map(|e| match e {
            LobbyEffect::CreateGame { .. } => Some(GameId::new()),
            _ => None,
        });

        self.process_effects(&lobby, effects).await;

        Ok(game_id)
    }

    pub async fn finalize_game_start(
        &self,
        lobby_id: LobbyId,
        game_id: GameId,
    ) -> Result<Vec<PlayerId>, LobbyServiceError> {
        let Some(mut lobby) = self.repository.load_lobby(lobby_id).await else {
            return Err(LobbyServiceError::LobbyNotFound(lobby_id));
        };

        let players = lobby.current_players();
        let effects = lobby.process_action(LobbyAction::StartGame(game_id));

        self.process_effects(&lobby, effects).await;

        // Clean up lobby after game starts
        self.repository.delete_lobby(lobby_id).await;

        Ok(players)
    }

    async fn process_effects(
        &self,
        lobby: &LobbyState,
        effects: Vec<LobbyEffect>,
    ) {
        for effect in effects {
            match effect {
                LobbyEffect::Notify { player_id, event } => {
                    self.notifier
                        .notify_player(player_id, LobbyNotification::LobbyEvent(event))
                        .await;
                }
                LobbyEffect::Broadcast { event } => {
                    for &player_id in &lobby.arrived_players {
                        self.notifier
                            .notify_player(player_id, LobbyNotification::LobbyEvent(event))
                            .await;
                    }
                }
                LobbyEffect::ScheduleCountdownTick { .. } => {
                    // Timer scheduling is handled by the caller (e.g., LobbyCountdownHandler)
                }
                LobbyEffect::CreateGame { .. } => {
                    // Game creation is handled by the caller (countdown_tick returns the game_id)
                }
            }
        }
    }

    pub async fn find_lobby_by_player(
        &self,
        player_id: PlayerId,
    ) -> Option<LobbyId> {
        self.repository.find_lobby_by_player(player_id).await
    }

    pub fn should_schedule_countdown(effects: &[LobbyEffect]) -> Option<u32> {
        effects.iter().find_map(|e| match e {
            LobbyEffect::ScheduleCountdownTick { delay_seconds } => Some(*delay_seconds),
            _ => None,
        })
    }

    async fn send_lobby_state(
        &self,
        lobby: &LobbyState,
        player_id: PlayerId,
    ) {
        let players: Vec<LobbyPlayerInfo> = lobby
            .arrived_players
            .iter()
            .map(|&pid| LobbyPlayerInfo {
                player_id: pid,
                is_ready: lobby.ready_players.contains(&pid),
            })
            .collect();

        let (phase, countdown_remaining) = match &lobby.phase {
            LobbyPhase::WaitingForReady => ("waiting".to_string(), None),
            LobbyPhase::CountingDown { remaining_seconds } => ("countdown".to_string(), Some(*remaining_seconds)),
            LobbyPhase::GameStarted { .. } => ("starting".to_string(), None),
            LobbyPhase::Cancelled => ("cancelled".to_string(), None),
        };

        self.notifier
            .notify_player(
                player_id,
                LobbyNotification::LobbyState {
                    lobby_id: lobby.id,
                    players,
                    phase,
                    countdown_remaining,
                },
            )
            .await;
    }
}

pub struct MatchmakingHandler<MN, MR, LN, LR, T> {
    matchmaking_notifier: MN,
    matchmaking_repository: MR,
    lobby_notifier: LN,
    lobby_repository: LR,
    timer: T,
    check_interval: Duration,
    required_players: usize,
}

impl<MN, MR, LN, LR, T> MatchmakingHandler<MN, MR, LN, LR, T>
where
    for<'a> &'a MN: MatchmakingEventNotifier,
    for<'a> &'a MR: MatchmakingQueueRepository,
    for<'a> &'a LN: LobbyEventNotifier,
    for<'a> &'a LR: LobbyRepository,
    T: AsyncTimer,
{
    pub fn new(
        matchmaking_notifier: MN,
        matchmaking_repository: MR,
        lobby_notifier: LN,
        lobby_repository: LR,
        timer: T,
        check_interval: Duration,
        required_players: usize,
    ) -> Self {
        Self {
            matchmaking_notifier,
            matchmaking_repository,
            lobby_notifier,
            lobby_repository,
            timer,
            check_interval,
            required_players,
        }
    }

    pub async fn run(&self) {
        loop {
            self.timer.sleep(self.check_interval).await;
            let _ = self.check_and_match().await;
        }
    }

    pub async fn check_and_match(&self) -> Option<LobbyId> {
        let queue: Vec<PlayerId> = (&self.matchmaking_repository).load_queue().await;

        if queue.len() >= self.required_players {
            // Take the first N players
            let matched_players: Vec<PlayerId> = queue.iter().take(self.required_players).copied().collect();

            // Create the lobby
            let lobby_service = LobbyService::new(&self.lobby_notifier, &self.lobby_repository);
            let lobby_id = lobby_service.create_lobby(matched_players.clone()).await.ok()?;

            // Notify matchmaking service about the match
            let matchmaking_service = MatchmakingService::new(&self.matchmaking_notifier, &self.matchmaking_repository);
            let _ = matchmaking_service.game_found(matched_players, lobby_id).await;

            Some(lobby_id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::InMemory;
    use crate::domain::ports::{GameNotification, GameRepository, MatchmakingQueueRepository};
    use crate::domain::{GameEvent, GameState, LobbyId};

    fn test_config() -> GameConfig {
        GameConfig {
            tick_interval_ms: 1000,
            max_price_delta: 10,
            starting_price: 50,
        }
    }

    fn create_test_game(
        players: Vec<PlayerId>,
        starting_balance: i32,
    ) -> GameState {
        let mut game = GameState::new(players, starting_balance, test_config());
        // Start the game so it's in Running state for tests
        game.process_action(GameAction::Start).unwrap();
        game
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
        // Give player some shares first by placing a bid high enough to always resolve
        // (starting price 50 + max delta 10 = 60 max, so bid of 100 always resolves)
        game.process_action(GameAction::Bid {
            player_id: player,
            bid_value: 100,
        })
        .unwrap();
        game.process_action(GameAction::PriceTick).unwrap(); // This resolves the bid
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
        let lobby_id = LobbyId::new();

        let service: MatchmakingService<&InMemory, &InMemory> = MatchmakingService::new(&adapter, &adapter);
        let _ = service.join_queue(player1).await;
        let _ = service.join_queue(player2).await;

        // Act
        let result = service.game_found(vec![player1, player2], lobby_id).await;

        // Assert
        assert!(result.is_ok());

        let events = adapter.get_matchmaking_events();
        let player1_got_lobby_created = events.iter().any(|(pid, notif)| {
            *pid == player1 && matches!(notif, MatchmakingNotification::LobbyCreated(lid) if *lid == lobby_id)
        });
        let player2_got_lobby_created = events.iter().any(|(pid, notif)| {
            *pid == player2 && matches!(notif, MatchmakingNotification::LobbyCreated(lid) if *lid == lobby_id)
        });
        assert!(player1_got_lobby_created, "Player1 should receive LobbyCreated notification");
        assert!(player2_got_lobby_created, "Player2 should receive LobbyCreated notification");
    }

    // ==================== GameService start_game/process_price_tick Tests ====================

    #[tokio::test]
    async fn test_start_game() {
        // Arrange
        let adapter = InMemory::new();
        let player = PlayerId::new();

        let mut service: GameService<&InMemory, &InMemory> = GameService::new(&adapter, &adapter);
        let game_id = service.new_game(vec![player], 1000, test_config()).await.unwrap();

        // Act
        let result = service.start_game(game_id).await;

        // Assert
        assert!(result.is_ok());
        let delay = result.unwrap();
        assert_eq!(delay, Some(1000)); // tick_interval_ms from test_config

        let events = adapter.get_game_events();
        let has_started = events
            .iter()
            .any(|(_, notif)| matches!(notif, GameNotification::GameEvent(GameEvent::GameStarted { .. })));
        assert!(has_started, "Expected GameStarted notification");
    }

    #[tokio::test]
    async fn test_process_price_tick() {
        // Arrange
        let adapter = InMemory::new();
        let player = PlayerId::new();

        let mut service: GameService<&InMemory, &InMemory> = GameService::new(&adapter, &adapter);
        let game_id = service.new_game(vec![player], 1000, test_config()).await.unwrap();
        service.start_game(game_id).await.unwrap();

        // Act
        let result = service.process_price_tick(game_id).await;

        // Assert
        assert!(result.is_ok());
        let delay = result.unwrap();
        assert_eq!(delay, Some(1000)); // Should schedule next tick

        let events = adapter.get_game_events();
        let has_price_changed = events
            .iter()
            .any(|(_, notif)| matches!(notif, GameNotification::GameEvent(GameEvent::PriceChanged(_))));
        assert!(has_price_changed, "Expected PriceChanged notification");
    }

    // ==================== LobbyService Tests ====================

    #[tokio::test]
    async fn test_create_lobby() {
        // Arrange
        let adapter = InMemory::new();
        let p1 = PlayerId::new();
        let p2 = PlayerId::new();

        let service: LobbyService<&InMemory, &InMemory> = LobbyService::new(&adapter, &adapter);

        // Act
        let result = service.create_lobby(vec![p1, p2]).await;

        // Assert
        assert!(result.is_ok());
        let lobby_id = result.unwrap();

        // Verify lobby was saved
        use crate::domain::ports::LobbyRepository;
        let lobby = adapter.load_lobby(lobby_id).await;
        assert!(lobby.is_some());
        let lobby = lobby.unwrap();
        assert_eq!(lobby.expected_players, vec![p1, p2]);
    }

    #[tokio::test]
    async fn test_player_arrived_notifies() {
        // Arrange
        let adapter = InMemory::new();
        let p1 = PlayerId::new();
        let p2 = PlayerId::new();

        let service: LobbyService<&InMemory, &InMemory> = LobbyService::new(&adapter, &adapter);
        let lobby_id = service.create_lobby(vec![p1, p2]).await.unwrap();

        // Act
        let result = service.player_arrived(lobby_id, p1).await;

        // Assert
        assert!(result.is_ok());
        let events = adapter.get_lobby_events();
        // Should have PlayerArrived broadcast + LobbyState sent to arriving player
        assert!(events.len() >= 2);
    }

    #[tokio::test]
    async fn test_player_ready_starts_countdown() {
        // Arrange
        let adapter = InMemory::new();
        let p1 = PlayerId::new();

        let service: LobbyService<&InMemory, &InMemory> = LobbyService::new(&adapter, &adapter);
        let lobby_id = service.create_lobby(vec![p1]).await.unwrap();

        service.player_arrived(lobby_id, p1).await.unwrap();

        // Act
        let result = service.player_ready(lobby_id, p1).await;

        // Assert
        assert!(result.is_ok());

        use crate::domain::ports::LobbyRepository;
        let lobby = adapter.load_lobby(lobby_id).await.unwrap();
        assert!(matches!(
            lobby.phase,
            crate::domain::LobbyPhase::CountingDown { remaining_seconds: 10 }
        ));

        // Verify countdown started notification was sent
        let events = adapter.get_lobby_events();
        let has_countdown_started = events.iter().any(|(_, notif)| {
            matches!(
                notif,
                LobbyNotification::LobbyEvent(crate::domain::LobbyEvent::CountdownStarted { seconds: 10 })
            )
        });
        assert!(has_countdown_started, "Expected CountdownStarted notification");
    }

    #[tokio::test]
    async fn test_lobby_not_found_error() {
        // Arrange
        let adapter = InMemory::new();
        let p1 = PlayerId::new();
        let fake_lobby_id = LobbyId::new();

        let service: LobbyService<&InMemory, &InMemory> = LobbyService::new(&adapter, &adapter);

        // Act
        let result = service.player_arrived(fake_lobby_id, p1).await;

        // Assert
        assert!(matches!(result, Err(LobbyServiceError::LobbyNotFound(_))));
    }

    #[tokio::test]
    async fn test_countdown_tick_decrements() {
        // Arrange
        let adapter = InMemory::new();
        let p1 = PlayerId::new();

        let service: LobbyService<&InMemory, &InMemory> = LobbyService::new(&adapter, &adapter);
        let lobby_id = service.create_lobby(vec![p1]).await.unwrap();

        service.player_arrived(lobby_id, p1).await.unwrap();
        service.player_ready(lobby_id, p1).await.unwrap();

        // Act - tick once
        let result = service.countdown_tick(lobby_id).await;

        // Assert
        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // No game created yet

        use crate::domain::ports::LobbyRepository;
        let lobby = adapter.load_lobby(lobby_id).await.unwrap();
        assert!(matches!(
            lobby.phase,
            crate::domain::LobbyPhase::CountingDown { remaining_seconds: 9 }
        ));
    }

    #[tokio::test]
    async fn test_countdown_complete_triggers_game_creation() {
        // Arrange
        let adapter = InMemory::new();
        let p1 = PlayerId::new();

        let service: LobbyService<&InMemory, &InMemory> = LobbyService::new(&adapter, &adapter);
        let lobby_id = service.create_lobby(vec![p1]).await.unwrap();

        service.player_arrived(lobby_id, p1).await.unwrap();
        service.player_ready(lobby_id, p1).await.unwrap();

        // Tick down to 1
        for _ in 0..9 {
            service.countdown_tick(lobby_id).await.unwrap();
        }

        // Act - final tick
        let result = service.countdown_tick(lobby_id).await;

        // Assert
        assert!(result.is_ok());
        assert!(result.unwrap().is_some()); // Game creation triggered
    }

    #[tokio::test]
    async fn test_finalize_game_start_cleans_up_lobby() {
        // Arrange
        let adapter = InMemory::new();
        let p1 = PlayerId::new();

        let service: LobbyService<&InMemory, &InMemory> = LobbyService::new(&adapter, &adapter);
        let lobby_id = service.create_lobby(vec![p1]).await.unwrap();

        service.player_arrived(lobby_id, p1).await.unwrap();

        let game_id = GameId::new();

        // Act
        let result = service.finalize_game_start(lobby_id, game_id).await;

        // Assert
        assert!(result.is_ok());
        let players = result.unwrap();
        assert!(players.contains(&p1));

        // Verify lobby was deleted
        use crate::domain::ports::LobbyRepository;
        let lobby = adapter.load_lobby(lobby_id).await;
        assert!(lobby.is_none());
    }
}
