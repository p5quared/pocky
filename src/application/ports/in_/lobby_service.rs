use crate::application::domain::{GameId, LobbyAction, LobbyEffect, LobbyId, LobbyPhase, LobbyState, PlayerId};
use crate::application::ports::out_::{LobbyEventNotifier, LobbyNotification, LobbyPlayerInfo, LobbyRepository, LobbyServiceError};

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
