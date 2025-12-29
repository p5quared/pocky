use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

use crate::domain::ports::{GameEventNotifier, GameNotification, GameRepository};
use crate::domain::{GameId, GameState, PlayerId};

pub struct InMemory {
    games: RwLock<HashMap<GameId, GameState>>,
    game_events: Mutex<Vec<(PlayerId, GameNotification)>>,
}

impl GameEventNotifier for InMemory {
    async fn notify_player(
        &mut self,
        player_id: PlayerId,
        notification: GameNotification,
    ) {
        self.game_events
            .lock()
            .expect("No mutex poisoning")
            .push((player_id, notification));
    }
}

impl InMemory {
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            game_events: Mutex::new(Vec::default()),
        }
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
