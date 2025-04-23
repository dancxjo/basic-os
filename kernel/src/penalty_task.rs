use alloc::{boxed::Box, string::String};

use crate::kernel::Task;

pub struct PenaltyTask {
    task: Box<dyn Task>,
    penalty_frames: u64,
    name: Option<String>,
}

impl PenaltyTask {
    pub fn new(task: Box<dyn Task>, name: Option<String>) -> Self {
        PenaltyTask {
            task,
            penalty_frames: 0,
            name,
        }
    }

    pub fn penalty_frames(&self) -> u64 {
        self.penalty_frames
    }

    pub fn reduce_penalty(&mut self) {
        if self.penalty_frames > 0 {
            self.penalty_frames -= 1;
        }
    }

    pub fn set_penalty(&mut self, frames: u64) {
        self.penalty_frames = frames;
    }

    pub fn tick(&mut self) {
        self.task.tick();
    }

    pub fn done(&mut self) -> bool {
        self.task.done()
    }

    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("Unnamed Task")
    }
}
