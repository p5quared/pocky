use super::PlayerId;

#[derive(Default)]
pub struct MatchmakingQueue(Vec<PlayerId>);

impl MatchmakingQueue {
    pub fn players(&self) -> &Vec<PlayerId> {
        &self.0
    }
}

pub enum MatchmakingCommand {
    PlayerJoin(PlayerId),
    PlayerLeave(PlayerId),
    TryMatchmake,
}

pub enum MatchmakingOutcome {
    Matched(Vec<PlayerId>),
    Enqueued(PlayerId),
    Dequeued(PlayerId),
    PlayerNotFound,
    AlreadyQueued,
}

impl MatchmakingQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(
        &mut self,
        command: MatchmakingCommand,
    ) -> MatchmakingOutcome {
        match command {
            MatchmakingCommand::PlayerJoin(player_id) => {
                if !self.0.contains(&player_id) {
                    self.0.push(player_id);
                    MatchmakingOutcome::Enqueued(player_id)
                } else {
                    MatchmakingOutcome::AlreadyQueued
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
                let mut events = Vec::new();
                while self.0.len() >= 2 {
                    let player1 = self.0.remove(0);
                    let player2 = self.0.remove(0);
                    events.push(player1);
                    events.push(player2);
                }
                MatchmakingOutcome::Matched(events)
            }
        }
    }
}
