use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerId(pub uuid::Uuid);

impl PlayerId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct GameId(pub uuid::Uuid);

impl GameId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct LobbyId(pub uuid::Uuid);

impl LobbyId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
