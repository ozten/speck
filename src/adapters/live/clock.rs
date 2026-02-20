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
