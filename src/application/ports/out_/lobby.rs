use serde::Serialize;

use crate::application::domain::{LobbyEvent, LobbyId, LobbyState, PlayerId};

#[derive(Debug)]
pub enum LobbyServiceError {
    LobbyNotFound(LobbyId),
    PlayerNotInLobby(PlayerId),
}

#[derive(Clone, Serialize)]
pub enum LobbyNotification {
    LobbyEvent(LobbyEvent),
    LobbyState {
        lobby_id: LobbyId,
        players: Vec<LobbyPlayerInfo>,
        phase: String,
        countdown_remaining: Option<u32>,
    },
}

#[derive(Clone, Serialize)]
pub struct LobbyPlayerInfo {
    pub player_id: PlayerId,
    pub is_ready: bool,
}

pub trait LobbyRepository {
    fn load_lobby(
        &self,
        lobby_id: LobbyId,
    ) -> impl Future<Output = Option<LobbyState>> + Send;

    fn save_lobby(
        &self,
        lobby_id: LobbyId,
        lobby_state: &LobbyState,
    ) -> impl Future<Output = ()> + Send;

    fn delete_lobby(
        &self,
        lobby_id: LobbyId,
    ) -> impl Future<Output = ()> + Send;

    fn find_lobby_by_player(
        &self,
        player_id: PlayerId,
    ) -> impl Future<Output = Option<LobbyId>> + Send;
}

pub trait LobbyEventNotifier {
    fn notify_player(
        &self,
        player_id: PlayerId,
        notification: LobbyNotification,
    ) -> impl Future<Output = ()> + Send;
}
