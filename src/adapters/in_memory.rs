use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use crate::domain::ports::{
    AsyncTimer, GameEventNotifier, GameNotification, GameRepository, LobbyEventNotifier, LobbyNotification, LobbyRepository,
    MatchmakingEventNotifier, MatchmakingNotification, MatchmakingQueueRepository,
};
use crate::domain::{GameId, GameState, LobbyId, LobbyState, PlayerId};

pub struct InMemory {
    games: RwLock<HashMap<GameId, GameState>>,
    game_events: RwLock<Vec<(PlayerId, GameNotification)>>,
    matchmaking_queue: RwLock<Vec<PlayerId>>,
    matchmaking_events: RwLock<Vec<(PlayerId, MatchmakingNotification)>>,
    lobbies: RwLock<HashMap<LobbyId, LobbyState>>,
    lobby_events: RwLock<Vec<(PlayerId, LobbyNotification)>>,
    player_lobby_map: RwLock<HashMap<PlayerId, LobbyId>>,
}

impl GameEventNotifier for InMemory {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: GameNotification,
    ) {
        self.game_events.write().unwrap().push((player_id, notification));
    }
}

impl GameEventNotifier for &InMemory {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: GameNotification,
    ) {
        self.game_events.write().unwrap().push((player_id, notification));
    }
}

impl InMemory {
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            game_events: RwLock::new(Vec::new()),
            matchmaking_queue: RwLock::new(Vec::new()),
            matchmaking_events: RwLock::new(Vec::new()),
            lobbies: RwLock::new(HashMap::new()),
            lobby_events: RwLock::new(Vec::new()),
            player_lobby_map: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_game_events(&self) -> Vec<(PlayerId, GameNotification)> {
        self.game_events.read().unwrap().clone()
    }

    pub fn get_matchmaking_events(&self) -> Vec<(PlayerId, MatchmakingNotification)> {
        self.matchmaking_events.read().unwrap().clone()
    }

    pub fn get_lobby_events(&self) -> Vec<(PlayerId, LobbyNotification)> {
        self.lobby_events.read().unwrap().clone()
    }
}

impl Default for InMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl GameRepository for InMemory {
    async fn load_game(
        &self,
        game_id: GameId,
    ) -> Option<GameState> {
        self.games.read().unwrap().get(&game_id).cloned()
    }

    async fn save_game(
        &self,
        game_id: GameId,
        game_state: &GameState,
    ) {
        self.games.write().unwrap().insert(game_id, game_state.clone());
    }
}

impl GameRepository for &InMemory {
    async fn load_game(
        &self,
        game_id: GameId,
    ) -> Option<GameState> {
        self.games.read().unwrap().get(&game_id).cloned()
    }

    async fn save_game(
        &self,
        game_id: GameId,
        game_state: &GameState,
    ) {
        self.games.write().unwrap().insert(game_id, game_state.clone());
    }
}

impl MatchmakingQueueRepository for InMemory {
    async fn load_queue(&self) -> Vec<PlayerId> {
        self.matchmaking_queue.read().unwrap().clone()
    }

    async fn save_queue(
        &self,
        queue: &Vec<PlayerId>,
    ) {
        *self.matchmaking_queue.write().unwrap() = queue.clone();
    }
}

impl MatchmakingQueueRepository for &InMemory {
    async fn load_queue(&self) -> Vec<PlayerId> {
        self.matchmaking_queue.read().unwrap().clone()
    }

    async fn save_queue(
        &self,
        queue: &Vec<PlayerId>,
    ) {
        *self.matchmaking_queue.write().unwrap() = queue.clone();
    }
}

impl MatchmakingEventNotifier for InMemory {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: MatchmakingNotification,
    ) {
        self.matchmaking_events.write().unwrap().push((player_id, notification));
    }
}

impl MatchmakingEventNotifier for &InMemory {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: MatchmakingNotification,
    ) {
        self.matchmaking_events.write().unwrap().push((player_id, notification));
    }
}

impl AsyncTimer for InMemory {
    async fn sleep(
        &self,
        _duration: Duration,
    ) {
        // No-op for testing - instant return
    }
}

impl AsyncTimer for &InMemory {
    async fn sleep(
        &self,
        _duration: Duration,
    ) {
        // No-op for testing - instant return
    }
}

impl LobbyRepository for InMemory {
    async fn load_lobby(
        &self,
        lobby_id: LobbyId,
    ) -> Option<LobbyState> {
        self.lobbies.read().unwrap().get(&lobby_id).cloned()
    }

    async fn save_lobby(
        &self,
        lobby_id: LobbyId,
        lobby_state: &LobbyState,
    ) {
        self.lobbies.write().unwrap().insert(lobby_id, lobby_state.clone());
        // Update player-to-lobby mapping
        let mut map = self.player_lobby_map.write().unwrap();
        for player in &lobby_state.arrived_players {
            map.insert(*player, lobby_id);
        }
    }

    async fn delete_lobby(
        &self,
        lobby_id: LobbyId,
    ) {
        if let Some(lobby) = self.lobbies.write().unwrap().remove(&lobby_id) {
            let mut map = self.player_lobby_map.write().unwrap();
            for player in &lobby.arrived_players {
                map.remove(player);
            }
        }
    }

    async fn find_lobby_by_player(
        &self,
        player_id: PlayerId,
    ) -> Option<LobbyId> {
        self.player_lobby_map.read().unwrap().get(&player_id).copied()
    }
}

impl LobbyRepository for &InMemory {
    async fn load_lobby(
        &self,
        lobby_id: LobbyId,
    ) -> Option<LobbyState> {
        self.lobbies.read().unwrap().get(&lobby_id).cloned()
    }

    async fn save_lobby(
        &self,
        lobby_id: LobbyId,
        lobby_state: &LobbyState,
    ) {
        self.lobbies.write().unwrap().insert(lobby_id, lobby_state.clone());
        // Update player-to-lobby mapping
        let mut map = self.player_lobby_map.write().unwrap();
        for player in &lobby_state.arrived_players {
            map.insert(*player, lobby_id);
        }
    }

    async fn delete_lobby(
        &self,
        lobby_id: LobbyId,
    ) {
        if let Some(lobby) = self.lobbies.write().unwrap().remove(&lobby_id) {
            let mut map = self.player_lobby_map.write().unwrap();
            for player in &lobby.arrived_players {
                map.remove(player);
            }
        }
    }

    async fn find_lobby_by_player(
        &self,
        player_id: PlayerId,
    ) -> Option<LobbyId> {
        self.player_lobby_map.read().unwrap().get(&player_id).copied()
    }
}

impl LobbyEventNotifier for InMemory {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: LobbyNotification,
    ) {
        self.lobby_events.write().unwrap().push((player_id, notification));
    }
}

impl LobbyEventNotifier for &InMemory {
    async fn notify_player(
        &self,
        player_id: PlayerId,
        notification: LobbyNotification,
    ) {
        self.lobby_events.write().unwrap().push((player_id, notification));
    }
}
