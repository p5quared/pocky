use std::sync::Arc;

use crate::ports::out_::{QueueNotifier, QueueRepository};
use domain::{MatchmakingCommand, MatchmakingOutcome, PlayerId};

pub struct MatchmakingService {
    repository: Arc<dyn QueueRepository>,
    notifier: Arc<dyn QueueNotifier>,
}

impl MatchmakingService {
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
        let players_before_matchmaking = q.players().clone();
        self.notifier.broadcast(&players_before_matchmaking, &event).await;
        if let MatchmakingOutcome::Matched(players) = q.execute(MatchmakingCommand::TryMatchmake)
            && !players.is_empty()
        {
            let matched = MatchmakingOutcome::Matched(players);
            self.notifier.broadcast(&players_before_matchmaking, &matched).await;
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
