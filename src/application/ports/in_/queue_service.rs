use crate::application::domain::{MatchmakingCommand, MatchmakingOutcome, PlayerId};
use crate::application::ports::out_::{QueueNotifier, QueueRepository};

pub struct MatchmakingQueueService<R: QueueRepository, N: QueueNotifier> {
    repository: R,
    notifier: N,
}

impl<R: QueueRepository, N: QueueNotifier> MatchmakingQueueService<R, N> {
    pub fn new(
        repository: R,
        notifier: N,
    ) -> Self {
        Self { repository, notifier }
    }

    pub fn add_player(
        &self,
        player_id: PlayerId,
    ) -> MatchmakingOutcome {
        let notifier = &self.notifier;
        self.repository.with_state(|q| {
            let event = q.execute(MatchmakingCommand::PlayerJoin(player_id));
            notifier.broadcast(q.players(), &event);
            event
        })
    }

    pub fn remove_player(
        &self,
        player_id: PlayerId,
    ) -> MatchmakingOutcome {
        let notifier = &self.notifier;
        self.repository.with_state(|q| {
            let event = q.execute(MatchmakingCommand::PlayerLeave(player_id));
            notifier.broadcast(q.players(), &event);
            event
        })
    }

    pub fn try_matchmake(&self) -> MatchmakingOutcome {
        let notifier = &self.notifier;
        self.repository.with_state(|q| {
            let event = q.execute(MatchmakingCommand::TryMatchmake);
            notifier.broadcast(q.players(), &event);
            event
        })
    }
}
