use std::sync::Arc;

use crate::ports::out_::QueueNotifier;
use domain::{MatchmakingCommand, MatchmakingOutcome, MatchmakingQueue, PlayerId};

pub struct MatchmakingService {
    queue: MatchmakingQueue,
    notifier: Arc<dyn QueueNotifier>,
}

impl MatchmakingService {
    pub fn new(notifier: Arc<dyn QueueNotifier>) -> Self {
        Self {
            queue: MatchmakingQueue::new(),
            notifier,
        }
    }

    pub async fn join_queue(
        &mut self,
        player_id: PlayerId,
    ) -> MatchmakingOutcome {
        let event = self.queue.handle_command(MatchmakingCommand::PlayerJoin(player_id));
        let players_before_matchmaking = self.queue.queue().to_vec();
        self.notifier.broadcast(&players_before_matchmaking, &event).await;
        if let MatchmakingOutcome::Matched(players) = self.queue.handle_command(MatchmakingCommand::TryMatchmake)
            && !players.is_empty()
        {
            let matched = MatchmakingOutcome::Matched(players);
            self.notifier.broadcast(&players_before_matchmaking, &matched).await;
            return matched;
        }
        event
    }

    pub async fn remove_player(
        &mut self,
        player_id: PlayerId,
    ) -> MatchmakingOutcome {
        let event = self.queue.handle_command(MatchmakingCommand::PlayerLeave(player_id));
        self.notifier.broadcast(self.queue.queue(), &event).await;
        event
    }
}
