use std::time::Duration;

use crate::application::ports::out_::AsyncTimer;

pub struct TokioTimer;

impl TokioTimer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TokioTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncTimer for TokioTimer {
    async fn sleep(
        &self,
        duration: Duration,
    ) {
        tokio::time::sleep(duration).await;
    }
}
