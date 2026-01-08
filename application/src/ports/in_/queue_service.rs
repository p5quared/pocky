use std::sync::Arc;

use crate::ports::out_::queue::{QueueNotifier, QueueRepository};
use domain::{MatchmakingCommand, MatchmakingOutcome, PlayerId};

pub struct MatchmakingQueueService {
    repository: Arc<dyn QueueRepository>,
    notifier: Arc<dyn QueueNotifier>,
}

impl MatchmakingQueueService {
    pub fn new(
        repository: Arc<dyn QueueRepository>,
        notifier: Arc<dyn QueueNotifier>,
    ) -> Self {
        Self { repository, notifier }
    }

    pub async fn join_queue(
        &self,
        player_id: PlayerId,
    ) -> MatchmakingOutcome {
        let mut q = self.repository.load().await;
        let event = q.execute(MatchmakingCommand::PlayerJoin(player_id));
        self.repository.save(q.clone()).await;
        self.notifier.broadcast(q.players(), &event).await;
        if let MatchmakingOutcome::Matched(players) = q.execute(MatchmakingCommand::TryMatchmake) {
            let matched = MatchmakingOutcome::Matched(players);
            self.notifier.broadcast(q.players(), &matched).await;
            self.repository.save(q).await;
            return matched;
        }
        self.repository.save(q).await;
        event
    }

    pub async fn remove_player(
        &self,
        player_id: PlayerId,
    ) -> MatchmakingOutcome {
        let mut q = self.repository.load().await;
        let event = q.execute(MatchmakingCommand::PlayerLeave(player_id));
        self.notifier.broadcast(q.players(), &event).await;
        event
    }
}
