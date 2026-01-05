use crate::application::domain::{MatchmakingOutcome, MatchmakingQueue, PlayerId};

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
