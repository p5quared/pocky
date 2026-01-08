use domain::{MatchmakingOutcome, MatchmakingQueue, PlayerId};

#[async_trait::async_trait]
pub trait QueueRepository: Send + Sync {
    async fn save(
        &self,
        queue: MatchmakingQueue,
    );
    async fn load(&self) -> MatchmakingQueue;
}

#[async_trait::async_trait]
pub trait QueueNotifier: Send + Sync {
    async fn broadcast(
        &self,
        players: &[PlayerId],
        event: &MatchmakingOutcome,
    );
}
