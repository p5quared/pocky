use crate::{GameStateCore, game_core::PlayerId, game_instance::GameId};

pub enum GameRepositoryError {
    NotFound,
    ConnectionError,
    Unknown,
}

pub trait GameRepository {
    fn save(
        &self,
        id: GameId,
        state: GameStateCore,
    ) -> Result<(), GameRepositoryError>;
    fn load(
        &self,
        id: GameId,
    ) -> Result<(), GameRepositoryError>;
}

pub enum GameNotifierError {
    SendFailed,
    Unknown,
}

pub trait GameNotifier {
    fn notify(
        &self,
        id: PlayerId,
        state: &GameStateCore,
    ) -> Result<(), GameNotifierError>;

    fn publish(
        &self,
        ids: Vec<PlayerId>,
        state: &GameStateCore,
    ) -> Result<(), GameNotifierError>;
}
