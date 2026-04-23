//! A drop-in debounce utility struct for delaying the execution of search queries
// on different Page enum types

use crate::config::Config;
use std::time::{Duration, Instant};

/// Fields needed to facilitate debounced queries
#[derive(Debug, Clone)]
pub struct Debouncer {
    triggered: Option<Instant>,
    delay: Duration,
}

// TODO: handle variable debounce based on Page enum
impl Debouncer {
    pub fn new(delay_ms: u64) -> Self {
        Self {
            triggered: None,
            delay: Duration::from_millis(delay_ms),
        }
    }

    /// Reset debounce timer
    pub fn reset(&mut self) {
        self.triggered = Some(Instant::now()); // Clear debounce timer
    }

    pub fn is_ready(&mut self) -> bool {
        if let Some(instant) = self.triggered
            && instant.elapsed() >= self.delay
        {
            self.triggered = None;
            return true;
        }
        false
    }
}

/// Trait policy implemented for each Page enum to opt into debounce queries
pub trait DebouncePolicy {
    /// Returns Some(delay_ms) if this page should debounce, None otherwise                        
    fn debounce_delay(&self, config: &Config) -> Option<Duration>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn debouncer_becomes_ready_after_delay_and_clears_itself() {
        let mut debouncer = Debouncer::new(10);

        assert!(!debouncer.is_ready());

        debouncer.reset();
        assert!(!debouncer.is_ready());

        thread::sleep(Duration::from_millis(15));
        assert!(debouncer.is_ready());
        assert!(!debouncer.is_ready());
    }

    #[test]
    fn debouncer_reset_restarts_the_timer() {
        let mut debouncer = Debouncer::new(20);

        debouncer.reset();
        thread::sleep(Duration::from_millis(10));
        debouncer.reset();
        thread::sleep(Duration::from_millis(10));
        assert!(!debouncer.is_ready());

        thread::sleep(Duration::from_millis(15));
        assert!(debouncer.is_ready());
    }
}
