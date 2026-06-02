use std::time::{Duration, Instant};

const CHROME_EXIT_DURATION: Duration = Duration::from_millis(150);

#[derive(Debug, Default)]
pub(super) struct ChromeVisibility {
    unmap_at: Option<Instant>,
}

impl ChromeVisibility {
    pub fn reset(&mut self) {
        self.unmap_at = None;
    }

    pub fn mapped(&mut self, hidden: bool, animate_exit: bool) -> bool {
        if !hidden {
            self.reset();
            return true;
        }

        if !animate_exit {
            self.reset();
            return false;
        }

        let deadline = *self
            .unmap_at
            .get_or_insert_with(|| Instant::now() + CHROME_EXIT_DURATION);
        Instant::now() < deadline
    }
}
