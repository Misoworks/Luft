use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RecoveryPolicy {
    limit: usize,
    window: Duration,
}

impl RecoveryPolicy {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self { limit, window }
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn window(&self) -> Duration {
        self.window
    }
}
