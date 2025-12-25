use crate::PlayerId;

pub struct MatchmakingService {
    players: Vec<PlayerId>,
}

impl MatchmakingService {
    pub fn new() -> Self {
        Self { players: Vec::new() }
    }

    pub fn add_player(
        &mut self,
        player_id: PlayerId,
    ) {
        self.players.push(player_id);
    }

    pub fn remove_player(
        &mut self,
        player_id: &PlayerId,
    ) {
        self.players.retain(|&id| id != *player_id);
    }

    pub fn list_players(&self) -> Vec<PlayerId> {
        self.players.clone()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_matchmaking() {
        let mut service = super::MatchmakingService::new();
        let players = vec![super::PlayerId::new(), super::PlayerId::new()];

        for player in &players {
            service.add_player(*player);
            assert!(service.list_players().contains(player));
        }

        let listed_players = service.list_players();
        for player in &players {
            assert!(listed_players.contains(player));
        }

        for player in &players {
            service.remove_player(player);
            assert!(!service.list_players().contains(player));
        }
    }
}
