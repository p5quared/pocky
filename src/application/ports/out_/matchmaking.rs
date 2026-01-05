use serde::Serialize;

use crate::application::domain::{LobbyId, MatchmakingOutcome, MatchmakingQueue, PlayerId};

pub enum MatchmakingServiceError {
    Foo, // TODO: Enumerate errors
}

#[derive(Clone, Serialize)]
pub enum MatchmakingNotification {
    PlayerJoinedQueue(PlayerId),
    PlayerLeftQueue(PlayerId),
    LobbyCreated(LobbyId),
}

pub trait MatchmakingQueueRepository {
    fn load_queue(&self) -> impl Future<Output = Vec<PlayerId>> + Send;
    fn save_queue(
        &self,
        queue: &Vec<PlayerId>,
    ) -> impl Future<Output = ()> + Send;
}

pub trait MatchmakingEventNotifier {
    fn notify_player(
        &self,
        player_id: PlayerId,
        notification: MatchmakingNotification,
    ) -> impl Future<Output = ()> + Send;
}

// Sync ports for queue use case
pub trait QueueRepository {
    fn save(
        &self,
        queue: MatchmakingQueue,
    );
    fn load(&self) -> MatchmakingQueue;
}

pub trait QueueNotifier {
    fn broadcast(
        &self,
        players: &[PlayerId],
        event: &MatchmakingOutcome,
    );
}
