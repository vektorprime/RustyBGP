use std::path::Component::ParentDir;
use std::time::{Instant, Duration};

pub struct Timer {
    current_time: Instant,
    max_time: u16,
}

impl Timer {
    pub fn new(max_time: u16) -> Self {
        Timer {
            current_time: Instant::now(),
            max_time
        }
    }
}