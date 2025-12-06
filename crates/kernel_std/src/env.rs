use crate::log::StdLog;
use crate::time::StdClock;
use thing_model::env::{Clock, Log};

#[derive(Clone, Debug, Default)]
pub struct StdEnv {
    log: StdLog,
    clock: StdClock,
}

impl StdEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn logger(&self) -> StdLog {
        self.log
    }

    pub fn clock(&self) -> StdClock {
        self.clock.clone()
    }
}

impl Log for StdEnv {
    fn info(&self, msg: &str) {
        self.log.info(msg);
    }

    fn warn(&self, msg: &str) {
        self.log.warn(msg);
    }

    fn error(&self, msg: &str) {
        self.log.error(msg);
    }
}

impl Clock for StdEnv {
    fn now_monotonic(&self) -> u64 {
        self.clock.now_monotonic()
    }
}
