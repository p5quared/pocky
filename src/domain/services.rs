use std::time::Duration;

use rand::Rng;

use crate::domain::ports::{
    LobbyEventNotifier, LobbyNotification, LobbyPlayerInfo, LobbyRepository, LobbyServiceError, MatchmakingEventNotifier,
    MatchmakingNotification, MatchmakingQueueRepository, MatchmakingServiceError,
};

use super::ports::{AsyncTimer, GameEventNotifier, GameNotification, GameRepository, GameServiceError};
use super::types::LobbyId;
use super::{GameAction, GameEffect, LobbyAction, LobbyEffect, LobbyPhase, LobbyState, PlayerId, types::GameId};

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

    /// Place a buy order for shares at the specified price.
    ///
    /// The bid will be held as an open order until the market price reaches or exceeds
    /// the bid value, at which point it will be automatically resolved. Notifies all
    /// players in the game of the bid placement.
    ///
    /// # Errors
    /// Returns `GameNotFound` if the game does not exist.
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

    /// Place a sell order for shares at the specified price.
    ///
    /// The ask will be held as an open order until the market price reaches or falls below
    /// the ask value, at which point it will be automatically resolved. Notifies all players
    /// in the game of the ask placement.
    ///
    /// # Errors
    /// Returns `GameNotFound` if the game does not exist.
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

    /// Initialize a new trading game with the specified players and starting cash balance.
    ///
    /// Creates a new game state with all players starting at the given balance and no
    /// open orders or share ownership. The game is immediately persisted to the repository.
    ///
    /// Returns the unique identifier for the newly created game.
    pub async fn new_game(
        &mut self,
        players: Vec<PlayerId>,
        starting_balance: i32,
    ) -> Result<GameId, GameServiceError> {
        let game_id = GameId::new();
        let game_state = super::GameState::init(players, starting_balance);
        self.repository.save_game(game_id, &game_state).await;
        Ok(game_id)
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

    /// Run the price ticker loop, continuously updating the market price at regular intervals.
    ///
    /// This is a long-running background task that:
    /// - Updates the price by a random delta each tick
    /// - Automatically resolves any bids/asks that match the new price
    /// - Notifies all players of price changes and order resolutions
    ///
    /// The loop runs indefinitely until an error occurs or the game is not found.
    ///
    /// # Errors
    /// Returns `GameNotFound` if the game is deleted during execution.
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

    /// Add a player to the matchmaking queue.
    ///
    /// The player is appended to the queue and all players currently in the queue
    /// (including the new player) are notified that a new player has joined.
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

    /// Remove a player from the matchmaking queue.
    ///
    /// The player is removed and all remaining players in the queue are notified
    /// that the player has left.
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

    /// Notify players that they have been matched and assigned to a lobby.
    ///
    /// Removes the matched players from the queue, notifies remaining players
    /// that the matched players have left, and notifies the matched players
    /// that they should join the specified lobby.
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

    /// Create a new lobby for the specified players.
    ///
    /// The lobby starts in the `WaitingForReady` phase with no players arrived yet.
    /// Players must explicitly join the lobby to be marked as arrived.
    ///
    /// Returns the unique identifier for the newly created lobby.
    pub async fn create_lobby(
        &self,
        players: Vec<PlayerId>,
    ) -> Result<LobbyId, LobbyServiceError> {
        let lobby_id = LobbyId::new();
        let lobby = LobbyState::new(lobby_id, players);
        self.repository.save_lobby(lobby_id, &lobby).await;
        Ok(lobby_id)
    }

    /// Mark a player as having joined the lobby.
    ///
    /// Broadcasts a `PlayerArrived` event to all players in the lobby and sends
    /// the current lobby state (player list, ready status, phase) to the arriving player.
    ///
    /// # Errors
    /// Returns `LobbyNotFound` if the lobby does not exist.
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

    /// Mark a player as ready to start the game.
    ///
    /// If all arrived players become ready, automatically starts a 10-second countdown.
    /// Broadcasts `PlayerReady` and potentially `CountdownStarted` events to all players.
    ///
    /// # Errors
    /// Returns `LobbyNotFound` if the lobby does not exist.
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

    /// Mark a player as no longer ready to start the game.
    ///
    /// If a countdown is in progress, it will be cancelled and the lobby returns
    /// to the `WaitingForReady` phase. Broadcasts `PlayerUnready` and potentially
    /// `CountdownCancelled` events to all players.
    ///
    /// # Errors
    /// Returns `LobbyNotFound` if the lobby does not exist.
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

    /// Handle a player disconnecting from the lobby.
    ///
    /// Removes the player from the lobby. If all players disconnect, the lobby is cancelled.
    /// If a countdown is in progress, it continues with the remaining players.
    /// Broadcasts `PlayerDisconnected` and potentially `LobbyCancelled` events.
    ///
    /// # Errors
    /// Returns `LobbyNotFound` if the lobby does not exist.
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

    /// Process a single tick of the countdown timer.
    ///
    /// Decrements the countdown by 1 second and broadcasts a `CountdownTick` event.
    /// When the countdown reaches 0, returns `Some(GameId)` to signal that the game
    /// should be created. The caller is responsible for creating the game and calling
    /// `finalize_game_start`.
    ///
    /// Returns `None` if the countdown is still in progress.
    ///
    /// # Errors
    /// Returns `LobbyNotFound` if the lobby does not exist.
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

    /// Finalize the transition from lobby to game.
    ///
    /// Broadcasts a `GameStarting` event to all players, deletes the lobby from
    /// the repository, and returns the list of players who should be in the game.
    ///
    /// This should be called after the game has been created via `GameService::new_game`.
    ///
    /// # Errors
    /// Returns `LobbyNotFound` if the lobby does not exist.
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

    /// Find the lobby that a player is currently in, if any.
    ///
    /// This is useful for handling player disconnects, where you need to determine
    /// which lobby to notify about the disconnection.
    pub async fn find_lobby_by_player(
        &self,
        player_id: PlayerId,
    ) -> Option<LobbyId> {
        self.repository.find_lobby_by_player(player_id).await
    }

    /// Check if countdown effects are present and return the delay if scheduling is needed.
    ///
    /// This is a utility method for external timer schedulers to determine if they
    /// need to schedule a countdown tick callback.
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

    /// Run the matchmaking handler loop as a background task.
    ///
    /// Continuously checks the matchmaking queue at regular intervals (`check_interval`).
    /// When enough players are waiting, automatically creates a lobby and notifies the
    /// matched players.
    ///
    /// This is a long-running task that should be spawned in the background (e.g., via `tokio::spawn`).
    pub async fn run(&self) {
        loop {
            self.timer.sleep(self.check_interval).await;
            let _ = self.check_and_match().await;
        }
    }

    /// Check the queue and create a lobby if enough players are waiting
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
