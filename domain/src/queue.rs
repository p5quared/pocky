use crate::PlayerId;

#[derive(Default, Clone)]
pub struct MatchmakingQueue(Vec<PlayerId>);

impl MatchmakingQueue {
    #[must_use]
    pub fn players(&self) -> &Vec<PlayerId> {
        &self.0
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
                if self.0.contains(&player_id) {
                    MatchmakingOutcome::AlreadyQueued
                } else {
                    self.0.push(player_id);
                    MatchmakingOutcome::Enqueued(player_id)
                }
            }
            MatchmakingCommand::PlayerLeave(player_id) => {
                if let Some(pos) = self.0.iter().position(|&pid| pid == player_id) {
                    self.0.remove(pos);
                    MatchmakingOutcome::Dequeued(player_id)
                } else {
                    MatchmakingOutcome::PlayerNotFound
                }
            }
            MatchmakingCommand::TryMatchmake => {
                if self.0.len() >= 2 {
                    let matched = vec![self.0.remove(0), self.0.remove(0)];
                    MatchmakingOutcome::Matched(matched)
                } else {
                    MatchmakingOutcome::Matched(vec![])
                }
            }
        }
    }
}
