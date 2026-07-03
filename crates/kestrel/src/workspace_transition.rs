use luft_ipc::WorkspaceId;
use std::time::{Duration, Instant};

const WORKSPACE_TRANSITION_DURATION: Duration = Duration::from_millis(220);

#[derive(Debug, Clone)]
pub struct WorkspaceTransition {
    from: WorkspaceId,
    to: WorkspaceId,
    direction: i32,
    started_at: Instant,
}

impl WorkspaceTransition {
    pub fn new(from: WorkspaceId, to: WorkspaceId, direction: i32) -> Self {
        Self {
            from,
            to,
            direction,
            started_at: Instant::now(),
        }
    }

    pub fn snapshot(&self) -> Option<WorkspaceTransitionSnapshot> {
        let elapsed = self.started_at.elapsed();
        if elapsed >= WORKSPACE_TRANSITION_DURATION {
            return None;
        }

        let progress = elapsed.as_secs_f64() / WORKSPACE_TRANSITION_DURATION.as_secs_f64();
        Some(WorkspaceTransitionSnapshot {
            from: self.from.clone(),
            to: self.to.clone(),
            direction: self.direction,
            progress: ease_workspace_transition(progress),
        })
    }

    pub fn is_active(&self) -> bool {
        self.started_at.elapsed() < WORKSPACE_TRANSITION_DURATION
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceTransitionSnapshot {
    pub from: WorkspaceId,
    pub to: WorkspaceId,
    pub direction: i32,
    pub progress: f64,
}

fn ease_workspace_transition(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    1.0 - (1.0 - progress).powi(3)
}
