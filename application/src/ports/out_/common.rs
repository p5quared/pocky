use std::time::Duration;

use async_trait::async_trait;

#[async_trait]
pub trait AsyncTimer: Send + Sync {
    async fn sleep(
        &self,
        duration: Duration,
    );
}
