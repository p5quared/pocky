use std::collections::HashSet;

use serde::Serialize;

use super::types::LobbyId;
use super::{GameId, PlayerId};

#[derive(Clone, Debug, PartialEq)]
pub enum LobbyPhase {
    WaitingForReady,
    CountingDown { remaining_seconds: u32 },
    GameStarted { game_id: GameId },
    Cancelled,
}

#[derive(Clone)]
pub struct LobbyState {
    pub id: LobbyId,
    pub expected_players: Vec<PlayerId>,
    pub arrived_players: HashSet<PlayerId>,
    pub ready_players: HashSet<PlayerId>,
    pub phase: LobbyPhase,
}

#[derive(Clone, Copy, Debug)]
pub enum LobbyAction {
    PlayerArrived(PlayerId),
    PlayerReady(PlayerId),
    PlayerUnready(PlayerId),
    PlayerDisconnected(PlayerId),
    CountdownTick,
    StartGame(GameId),
}

#[derive(Clone, Copy, Serialize, Debug, PartialEq)]
pub enum LobbyEvent {
    PlayerArrived(PlayerId),
    PlayerReady(PlayerId),
    PlayerUnready(PlayerId),
    PlayerDisconnected(PlayerId),
    CountdownStarted { seconds: u32 },
    CountdownTick { remaining_seconds: u32 },
    CountdownCancelled,
    GameStarting { game_id: GameId },
    LobbyCancelled,
}

#[derive(Clone, Debug)]
pub enum LobbyEffect {
    Notify { player_id: PlayerId, event: LobbyEvent },
    Broadcast { event: LobbyEvent },
    ScheduleCountdownTick { delay_seconds: u32 },
    CreateGame { lobby_id: LobbyId, players: Vec<PlayerId> },
}

impl LobbyState {
    pub fn new(
        id: LobbyId,
        expected_players: Vec<PlayerId>,
    ) -> Self {
        Self {
            id,
            expected_players,
            arrived_players: HashSet::new(),
            ready_players: HashSet::new(),
            phase: LobbyPhase::WaitingForReady,
        }
    }

    pub fn process_action(
        &mut self,
        action: LobbyAction,
    ) -> Vec<LobbyEffect> {
        match action {
            LobbyAction::PlayerArrived(player_id) => self.handle_player_arrived(player_id),
            LobbyAction::PlayerReady(player_id) => self.handle_player_ready(player_id),
            LobbyAction::PlayerUnready(player_id) => self.handle_player_unready(player_id),
            LobbyAction::PlayerDisconnected(player_id) => self.handle_player_disconnected(player_id),
            LobbyAction::CountdownTick => self.handle_countdown_tick(),
            LobbyAction::StartGame(game_id) => self.handle_start_game(game_id),
        }
    }

    pub fn current_players(&self) -> Vec<PlayerId> {
        self.arrived_players.iter().copied().collect()
    }

    fn handle_player_arrived(
        &mut self,
        player_id: PlayerId,
    ) -> Vec<LobbyEffect> {
        if !self.expected_players.contains(&player_id) {
            return vec![];
        }

        self.arrived_players.insert(player_id);

        vec![LobbyEffect::Broadcast {
            event: LobbyEvent::PlayerArrived(player_id),
        }]
    }

    fn handle_player_ready(
        &mut self,
        player_id: PlayerId,
    ) -> Vec<LobbyEffect> {
        if !self.arrived_players.contains(&player_id) {
            return vec![];
        }

        if !matches!(self.phase, LobbyPhase::WaitingForReady) {
            return vec![];
        }

        self.ready_players.insert(player_id);

        let mut effects = vec![LobbyEffect::Broadcast {
            event: LobbyEvent::PlayerReady(player_id),
        }];

        if self.all_players_ready() {
            self.phase = LobbyPhase::CountingDown { remaining_seconds: 10 };
            effects.push(LobbyEffect::Broadcast {
                event: LobbyEvent::CountdownStarted { seconds: 10 },
            });
            effects.push(LobbyEffect::ScheduleCountdownTick { delay_seconds: 1 });
        }

        effects
    }

    fn handle_player_unready(
        &mut self,
        player_id: PlayerId,
    ) -> Vec<LobbyEffect> {
        if !self.ready_players.remove(&player_id) {
            return vec![];
        }

        let mut effects = vec![LobbyEffect::Broadcast {
            event: LobbyEvent::PlayerUnready(player_id),
        }];

        if matches!(self.phase, LobbyPhase::CountingDown { .. }) {
            self.phase = LobbyPhase::WaitingForReady;
            effects.push(LobbyEffect::Broadcast {
                event: LobbyEvent::CountdownCancelled,
            });
        }

        effects
    }

    fn handle_player_disconnected(
        &mut self,
        player_id: PlayerId,
    ) -> Vec<LobbyEffect> {
        self.arrived_players.remove(&player_id);
        self.ready_players.remove(&player_id);

        let mut effects = vec![LobbyEffect::Broadcast {
            event: LobbyEvent::PlayerDisconnected(player_id),
        }];

        if self.arrived_players.is_empty() {
            self.phase = LobbyPhase::Cancelled;
            effects.push(LobbyEffect::Broadcast {
                event: LobbyEvent::LobbyCancelled,
            });
        }
        // Per requirements: if player disconnects during countdown, proceed anyway

        effects
    }

    fn handle_countdown_tick(&mut self) -> Vec<LobbyEffect> {
        match &mut self.phase {
            LobbyPhase::CountingDown { remaining_seconds } => {
                if *remaining_seconds > 1 {
                    *remaining_seconds -= 1;
                    vec![
                        LobbyEffect::Broadcast {
                            event: LobbyEvent::CountdownTick {
                                remaining_seconds: *remaining_seconds,
                            },
                        },
                        LobbyEffect::ScheduleCountdownTick { delay_seconds: 1 },
                    ]
                } else {
                    // Countdown complete - trigger game creation
                    let players: Vec<PlayerId> = self.arrived_players.iter().copied().collect();
                    vec![LobbyEffect::CreateGame {
                        lobby_id: self.id,
                        players,
                    }]
                }
            }
            _ => vec![],
        }
    }

    fn handle_start_game(
        &mut self,
        game_id: GameId,
    ) -> Vec<LobbyEffect> {
        self.phase = LobbyPhase::GameStarted { game_id };
        vec![LobbyEffect::Broadcast {
            event: LobbyEvent::GameStarting { game_id },
        }]
    }

    fn all_players_ready(&self) -> bool {
        !self.arrived_players.is_empty() && self.arrived_players.iter().all(|p| self.ready_players.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_lobby() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let p2 = PlayerId::new();

        let lobby = LobbyState::new(lobby_id, vec![p1, p2]);

        assert_eq!(lobby.id, lobby_id);
        assert_eq!(lobby.expected_players, vec![p1, p2]);
        assert!(lobby.arrived_players.is_empty());
        assert!(lobby.ready_players.is_empty());
        assert_eq!(lobby.phase, LobbyPhase::WaitingForReady);
    }

    #[test]
    fn test_player_arrived() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1]);

        let effects = lobby.process_action(LobbyAction::PlayerArrived(p1));

        assert!(lobby.arrived_players.contains(&p1));
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            &effects[0],
            LobbyEffect::Broadcast { event: LobbyEvent::PlayerArrived(pid) } if *pid == p1
        ));
    }

    #[test]
    fn test_unexpected_player_ignored() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let unexpected = PlayerId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1]);

        let effects = lobby.process_action(LobbyAction::PlayerArrived(unexpected));

        assert!(lobby.arrived_players.is_empty());
        assert!(effects.is_empty());
    }

    #[test]
    fn test_player_ready_starts_countdown() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let p2 = PlayerId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1, p2]);

        lobby.process_action(LobbyAction::PlayerArrived(p1));
        lobby.process_action(LobbyAction::PlayerArrived(p2));
        lobby.process_action(LobbyAction::PlayerReady(p1));
        let effects = lobby.process_action(LobbyAction::PlayerReady(p2));

        assert!(matches!(lobby.phase, LobbyPhase::CountingDown { remaining_seconds: 10 }));
        assert!(effects.iter().any(|e| matches!(
            e,
            LobbyEffect::Broadcast {
                event: LobbyEvent::CountdownStarted { seconds: 10 }
            }
        )));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, LobbyEffect::ScheduleCountdownTick { delay_seconds: 1 }))
        );
    }

    #[test]
    fn test_unready_cancels_countdown() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1]);

        lobby.process_action(LobbyAction::PlayerArrived(p1));
        lobby.process_action(LobbyAction::PlayerReady(p1));
        assert!(matches!(lobby.phase, LobbyPhase::CountingDown { .. }));

        let effects = lobby.process_action(LobbyAction::PlayerUnready(p1));

        assert_eq!(lobby.phase, LobbyPhase::WaitingForReady);
        assert!(effects.iter().any(|e| matches!(
            e,
            LobbyEffect::Broadcast {
                event: LobbyEvent::CountdownCancelled
            }
        )));
    }

    #[test]
    fn test_countdown_tick() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1]);

        lobby.process_action(LobbyAction::PlayerArrived(p1));
        lobby.process_action(LobbyAction::PlayerReady(p1));

        let effects = lobby.process_action(LobbyAction::CountdownTick);

        assert!(matches!(lobby.phase, LobbyPhase::CountingDown { remaining_seconds: 9 }));
        assert!(effects.iter().any(|e| matches!(
            e,
            LobbyEffect::Broadcast {
                event: LobbyEvent::CountdownTick { remaining_seconds: 9 }
            }
        )));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, LobbyEffect::ScheduleCountdownTick { delay_seconds: 1 }))
        );
    }

    #[test]
    fn test_countdown_complete_creates_game() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1]);

        lobby.process_action(LobbyAction::PlayerArrived(p1));
        lobby.process_action(LobbyAction::PlayerReady(p1));

        // Tick down to 1
        for _ in 0..9 {
            lobby.process_action(LobbyAction::CountdownTick);
        }

        assert!(matches!(lobby.phase, LobbyPhase::CountingDown { remaining_seconds: 1 }));

        // Final tick triggers game creation
        let effects = lobby.process_action(LobbyAction::CountdownTick);

        assert!(effects.iter().any(
            |e| matches!(e, LobbyEffect::CreateGame { lobby_id: lid, players } if *lid == lobby_id && players.contains(&p1))
        ));
    }

    #[test]
    fn test_disconnect_during_countdown_proceeds() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let p2 = PlayerId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1, p2]);

        lobby.process_action(LobbyAction::PlayerArrived(p1));
        lobby.process_action(LobbyAction::PlayerArrived(p2));
        lobby.process_action(LobbyAction::PlayerReady(p1));
        lobby.process_action(LobbyAction::PlayerReady(p2));

        assert!(matches!(lobby.phase, LobbyPhase::CountingDown { .. }));

        // p2 disconnects during countdown
        lobby.process_action(LobbyAction::PlayerDisconnected(p2));

        // Should still be counting down with remaining player
        assert!(matches!(lobby.phase, LobbyPhase::CountingDown { .. }));
        assert!(lobby.arrived_players.contains(&p1));
        assert!(!lobby.arrived_players.contains(&p2));
    }

    #[test]
    fn test_all_players_disconnect_cancels() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1]);

        lobby.process_action(LobbyAction::PlayerArrived(p1));
        let effects = lobby.process_action(LobbyAction::PlayerDisconnected(p1));

        assert_eq!(lobby.phase, LobbyPhase::Cancelled);
        assert!(effects.iter().any(|e| matches!(
            e,
            LobbyEffect::Broadcast {
                event: LobbyEvent::LobbyCancelled
            }
        )));
    }

    #[test]
    fn test_start_game() {
        let lobby_id = LobbyId::new();
        let p1 = PlayerId::new();
        let game_id = GameId::new();
        let mut lobby = LobbyState::new(lobby_id, vec![p1]);

        lobby.process_action(LobbyAction::PlayerArrived(p1));

        let effects = lobby.process_action(LobbyAction::StartGame(game_id));

        assert_eq!(lobby.phase, LobbyPhase::GameStarted { game_id });
        assert!(effects.iter().any(
            |e| matches!(e, LobbyEffect::Broadcast { event: LobbyEvent::GameStarting { game_id: gid } } if *gid == game_id)
        ));
    }
}
