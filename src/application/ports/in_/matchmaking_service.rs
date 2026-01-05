use std::time::Duration;

use crate::application::domain::{LobbyId, PlayerId};
use crate::application::ports::out_::{
    AsyncTimer, LobbyEventNotifier, LobbyRepository, MatchmakingEventNotifier, MatchmakingNotification,
    MatchmakingQueueRepository, MatchmakingServiceError,
};

use super::LobbyService;

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
