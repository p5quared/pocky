use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerId(pub uuid::Uuid);

impl Default for PlayerId {
    fn default() -> Self {
        PlayerId::new()
    }
}

impl PlayerId {
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct GameId(pub uuid::Uuid);

impl GameId {
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for GameId {
    fn default() -> Self {
        GameId::new()
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct LobbyId(pub uuid::Uuid);

impl LobbyId {
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for LobbyId {
    fn default() -> Self {
        LobbyId::new()
    }
}
