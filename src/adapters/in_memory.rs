use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use crate::domain::ports::{
    AsyncTimer, GameEventNotifier, GameNotification, GameRepository, MatchmakingEventNotifier, MatchmakingNotification,
    MatchmakingQueueRepository,
};
use crate::domain::{GameId, GameState, PlayerId};

pub struct InMemory {
    games: RwLock<HashMap<GameId, GameState>>,
    game_events: RwLock<Vec<(PlayerId, GameNotification)>>,
    matchmaking_queue: RwLock<Vec<PlayerId>>,
    matchmaking_events: RwLock<Vec<(PlayerId, MatchmakingNotification)>>,
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
        }
    }

    pub fn get_game_events(&self) -> Vec<(PlayerId, GameNotification)> {
        self.game_events.read().unwrap().clone()
    }

    pub fn get_matchmaking_events(&self) -> Vec<(PlayerId, MatchmakingNotification)> {
        self.matchmaking_events.read().unwrap().clone()
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
