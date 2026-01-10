use std::time::Duration;

use async_trait::async_trait;

use application::ports::out_::AsyncTimer;

pub struct TokioTimer;

impl TokioTimer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TokioTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncTimer for TokioTimer {
    async fn sleep(
        &self,
        duration: Duration,
    ) {
        tokio::time::sleep(duration).await;
    }
}
