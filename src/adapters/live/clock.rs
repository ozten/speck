//! Live clock using the system clock.

use chrono::{DateTime, Utc};

use crate::ports::clock::Clock;

/// Live clock that returns the real current time.
pub struct LiveClock;

impl Clock for LiveClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_current_time() {
        let clock = LiveClock;
        let before = Utc::now();
        let now = clock.now();
        let after = Utc::now();

        assert!(now >= before);
        assert!(now <= after);
    }
}
