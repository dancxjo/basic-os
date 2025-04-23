use crate::{clock::Clock, penalty_task::PenaltyTask};
use alloc::vec::Vec;
use log::warn;

pub struct Scheduler {
    tasks: Vec<PenaltyTask>,
    left_off_at: usize,
    frame_budget_ns: u64,
}

impl Scheduler {
    pub fn new(frame_budget_ns: u64) -> Self {
        Scheduler {
            tasks: Vec::new(),
            left_off_at: 0,
            frame_budget_ns,
        }
    }

    pub fn add_task(&mut self, task: PenaltyTask) {
        self.tasks.push(task);
    }

    pub fn tick(&mut self, clock: &Clock) {
        let mut i = self.left_off_at;

        while i < self.tasks.len() {
            let task = &mut self.tasks[i];

            if task.penalty_frames() > 0 {
                task.reduce_penalty();
                i += 1;
                continue;
            }

            let before = clock.nanos_since_boot();
            task.tick();
            let after = clock.nanos_since_boot();

            let elapsed = after - before;
            if elapsed > self.frame_budget_ns {
                let over = elapsed - self.frame_budget_ns;
                let penalty = over / self.frame_budget_ns + 1;
                task.set_penalty(penalty);

                warn!(
                    "Task '{}' overran budget by {}ns ({} frame penalty)",
                    task.name(),
                    over,
                    penalty
                );
            }

            if task.done() {
                self.tasks.remove(i);
            } else {
                i += 1;
            }
        }

        // Save where we left off
        self.left_off_at = if i >= self.tasks.len() { 0 } else { i };
    }
}
