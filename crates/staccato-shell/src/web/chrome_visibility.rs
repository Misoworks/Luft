use std::time::{Duration, Instant};

const CHROME_EXIT_DURATION: Duration = Duration::from_millis(150);

#[derive(Debug, Default)]
pub(super) struct ChromeVisibility {
    unmap_at: Option<Instant>,
}

impl ChromeVisibility {
    pub fn mapped(&mut self, hidden: bool) -> bool {
        if !hidden {
            self.unmap_at = None;
            return true;
        }

        let deadline = *self
            .unmap_at
            .get_or_insert_with(|| Instant::now() + CHROME_EXIT_DURATION);
        Instant::now() < deadline
    }
}
