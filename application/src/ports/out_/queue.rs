use domain::{MatchmakingOutcome, PlayerId};

#[async_trait::async_trait]
pub trait QueueNotifier: Send + Sync {
    async fn broadcast(
        &self,
        players: &[PlayerId],
        event: &MatchmakingOutcome,
    );
}
