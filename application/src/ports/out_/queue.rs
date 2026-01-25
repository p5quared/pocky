use domain::MatchmakingOutcome;

#[async_trait::async_trait]
pub trait QueueNotifier: Send + Sync {
    async fn broadcast(
        &self,
        event: &MatchmakingOutcome,
    );
}
