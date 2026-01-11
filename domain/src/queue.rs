use crate::PlayerId;

#[derive(Default, Clone)]
pub struct MatchmakingQueue {
    queue: Vec<PlayerId>,
    config: MatchmakingConfig,
}

#[derive(Clone)]
pub struct MatchmakingConfig {
    players_to_start: usize,
}

impl Default for MatchmakingConfig {
    fn default() -> Self {
        Self { players_to_start: 2 }
    }
}

impl MatchmakingConfig {
    pub fn players_to_start(&self) -> usize {
        self.players_to_start
    }
}

impl MatchmakingQueue {
    #[must_use]
    pub fn queue(&self) -> &Vec<PlayerId> {
        &self.queue
    }

    pub fn queue_mut(&mut self) -> &mut Vec<PlayerId> {
        &mut self.queue
    }
}

pub enum MatchmakingCommand {
    PlayerJoin(PlayerId),
    PlayerLeave(PlayerId),
    TryMatchmake,
}

#[derive(serde::Serialize, Debug)]
pub enum MatchmakingOutcome {
    Matched(Vec<PlayerId>),
    Enqueued(PlayerId),
    Dequeued(PlayerId),
    PlayerNotFound,
    AlreadyQueued,
}

impl MatchmakingQueue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_command(
        &mut self,
        command: MatchmakingCommand,
    ) -> MatchmakingOutcome {
        match command {
            MatchmakingCommand::PlayerJoin(player_id) => {
                if self.queue().contains(&player_id) {
                    MatchmakingOutcome::AlreadyQueued
                } else {
                    self.queue_mut().push(player_id);
                    MatchmakingOutcome::Enqueued(player_id)
                }
            }
            MatchmakingCommand::PlayerLeave(player_id) => {
                if let Some(pos) = self.queue().iter().position(|&pid| pid == player_id) {
                    self.queue_mut().remove(pos);
                    MatchmakingOutcome::Dequeued(player_id)
                } else {
                    MatchmakingOutcome::PlayerNotFound
                }
            }
            MatchmakingCommand::TryMatchmake => {
                if self.queue().len() >= self.config.players_to_start() {
                    let matched = vec![self.queue_mut().remove(0), self.queue_mut().remove(0)];
                    MatchmakingOutcome::Matched(matched)
                } else {
                    MatchmakingOutcome::Matched(vec![])
                }
            }
        }
    }
}
