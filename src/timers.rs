use std::path::Component::ParentDir;
use std::time::{Instant, Duration};
use crate::errors::TimerError;

#[derive(Debug)]
pub struct Timer {
    current_time: Option<Instant>,
    pub max_time: Duration,
}

impl Timer {
    pub fn new(max_time_sec: u16) -> Self {
        Timer {
            current_time: None,
            max_time: Duration::from_secs(max_time_sec as u64),
        }
    }

    pub fn set_max_time(&mut self, max_time_sec: u16) {
        self.max_time = Duration::from_secs(max_time_sec as u64);
    }

    pub fn is_elapsed(&self) -> Result<bool, TimerError> {
        if let Some(ct) = self.current_time {
            if ct.elapsed() >= self.max_time {
                Ok(true)
            }
            else {
                Ok(false)
            }
        } else {
            return Err(TimerError::TimerNotStarted)
        }
    }

    pub fn is_running(&self) -> Result<bool, TimerError> {
        // if let Some(_) = self.current_time {
        //     true
        // }
        // else {
        //     false
        // }
        if let Some(ct) = self.current_time {
            if ct.elapsed() >= self.max_time {
                Ok(false)
            }
            else {
                Ok(true)
            }
        } else {
            return Err(TimerError::TimerNotStarted)
        }
    }


    pub fn start(&mut self, new_max_time_sec: u16) {

        self.max_time = Duration::from_secs(new_max_time_sec as u64);

        self.current_time = Some(Instant::now());
    }

    pub fn stop(&mut self) {
        self.current_time = None;
    }
}

impl Default for Timer {
    fn default() -> Self {
        Timer {
            current_time: None,
            max_time: Duration::from_secs(0 as u64),
        }
    }
}