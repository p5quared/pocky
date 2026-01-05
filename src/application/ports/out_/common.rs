use std::time::Duration;

pub trait AsyncTimer {
    fn sleep(
        &self,
        duration: Duration,
    ) -> impl Future<Output = ()> + Send;
}
