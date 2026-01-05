use super::PlayerId;

pub struct MatchmakingQueue(Vec<PlayerId>);

impl MatchmakingQueue {
    pub fn players(&self) -> &[PlayerId] {
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
    Queued(PlayerId),
    Dequeued(PlayerId),
    PlayerNotFound,
    AlreadyQueued,
}

pub fn execute(
    queue: &mut MatchmakingQueue,
    command: MatchmakingCommand,
) -> MatchmakingOutcome {
    match command {
        MatchmakingCommand::PlayerJoin(player_id) => {
            if !queue.0.contains(&player_id) {
                queue.0.push(player_id);
                MatchmakingOutcome::Queued(player_id)
            } else {
                MatchmakingOutcome::AlreadyQueued
            }
        }
        MatchmakingCommand::PlayerLeave(player_id) => {
            if let Some(pos) = queue.0.iter().position(|&pid| pid == player_id) {
                queue.0.remove(pos);
                MatchmakingOutcome::Dequeued(player_id)
            } else {
                MatchmakingOutcome::PlayerNotFound
            }
        }
        MatchmakingCommand::TryMatchmake => {
            let mut events = Vec::new();
            while queue.0.len() >= 2 {
                let player1 = queue.0.remove(0);
                let player2 = queue.0.remove(0);
                events.push(player1);
                events.push(player2);
            }
            MatchmakingOutcome::Matched(events)
        }
    }
}
