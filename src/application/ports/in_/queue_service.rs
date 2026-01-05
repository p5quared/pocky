use crate::application::{
    domain::{MatchmakingCommand, MatchmakingOutcome, PlayerId},
    ports::out_::queue::{QueueNotifier, QueueRepository},
};

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
        let mut q = self.repository.load();
        let event = q.execute(MatchmakingCommand::PlayerJoin(player_id));
        self.notifier.broadcast(q.players(), &event);
        event
    }

    pub fn remove_player(
        &self,
        player_id: PlayerId,
    ) -> MatchmakingOutcome {
        let mut q = self.repository.load();
        let event = q.execute(MatchmakingCommand::PlayerLeave(player_id));
        self.notifier.broadcast(q.players(), &event);
        event
    }

    pub fn try_matchmake(&self) -> MatchmakingOutcome {
        let mut q = self.repository.load();
        let event = q.execute(MatchmakingCommand::TryMatchmake);
        self.notifier.broadcast(q.players(), &event);
        event
    }
}
