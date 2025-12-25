#[derive(Debug, PartialEq, Clone, Copy)]
pub struct PlayerId(pub uuid::Uuid);

impl PlayerId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
