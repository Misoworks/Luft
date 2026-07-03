use std::{
    process::Child,
    time::{Duration, Instant},
};
use tracing::warn;

pub(crate) struct LaunchedProcess {
    pub(super) command: String,
    pub(super) child: Child,
    started_at: Instant,
}

impl LaunchedProcess {
    pub(super) fn new(command: String, child: Child) -> Self {
        Self {
            command,
            child,
            started_at: Instant::now(),
        }
    }

    pub(super) fn is_running_or_report_exit(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() || self.started_at.elapsed() < Duration::from_secs(2) {
                    warn!(
                        command = %self.command,
                        %status,
                        "launched app exited"
                    );
                }
                false
            }
            Ok(None) => true,
            Err(error) => {
                warn!(command = %self.command, %error, "failed to poll launched app");
                false
            }
        }
    }
}
