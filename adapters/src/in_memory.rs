use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use async_trait::async_trait;

use application::ports::out_::queue::QueueRepository;
use application::ports::out_::{AsyncTimer, GameEventNotifier, GameEventScheduler, GameNotification, GameRepository};
use domain::{GameAction, GameId, GameState, MatchmakingQueue, PlayerId};

pub struct InMemory {
    games: RwLock<HashMap<GameId, GameState>>,
    game_events: RwLock<Vec<(PlayerId, GameNotification)>>,
    scheduled_actions: RwLock<Vec<(GameId, Duration, GameAction)>>,
}

#[async_trait]
impl GameEventNotifier for InMemory {
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
            scheduled_actions: RwLock::new(Vec::new()),
        }
    }

    pub fn get_game_events(&self) -> Vec<(PlayerId, GameNotification)> {
        self.game_events.read().unwrap().clone()
    }

    pub fn get_scheduled_actions(&self) -> Vec<(GameId, Duration, GameAction)> {
        self.scheduled_actions.read().unwrap().clone()
    }
}

impl Default for InMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
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

#[async_trait]
impl AsyncTimer for InMemory {
    async fn sleep(
        &self,
        _duration: Duration,
    ) {
        // No-op for testing - instant return
    }
}

#[async_trait]
impl GameEventScheduler for InMemory {
    async fn schedule_action(
        &self,
        game_id: GameId,
        delay: Duration,
        action: GameAction,
    ) {
        self.scheduled_actions.write().unwrap().push((game_id, delay, action));
    }
}

pub struct InMemoryQueueRepository {
    queue: RwLock<MatchmakingQueue>,
}

impl InMemoryQueueRepository {
    pub fn new() -> Self {
        Self {
            queue: RwLock::new(MatchmakingQueue::new()),
        }
    }
}

impl Default for InMemoryQueueRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QueueRepository for InMemoryQueueRepository {
    async fn load(&self) -> MatchmakingQueue {
        self.queue.read().unwrap().clone()
    }

    async fn save(
        &self,
        queue: MatchmakingQueue,
    ) {
        *self.queue.write().unwrap() = queue;
    }
}
