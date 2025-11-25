use crate::mm::allocator::{BootFrameAllocator, global_mapper};
use crate::task::context::TaskMode;
use crate::task::scheduler::{SCHEDULER, Scheduler};

pub type TaskId = usize;

/// Handle to a spawned task. Currently just wraps an index in the scheduler.
#[derive(Clone, Copy, Debug)]
pub struct TaskHandle {
    id: TaskId,
}

impl TaskHandle {
    pub const fn id(self) -> TaskId {
        self.id
    }
}

fn with_scheduler<R>(f: impl FnOnce(&mut Scheduler) -> R) -> R {
    let mut sched = SCHEDULER.lock();
    f(&mut sched)
}

/// Spawn a new kernel-mode task using the global mapper/frame allocator.
pub fn spawn_kernel(entry: extern "C" fn()) -> TaskHandle {
    let mapper = global_mapper();
    let frame_allocator = BootFrameAllocator::global();
    with_scheduler(|sched| {
        let id = sched.tasks.len();
        sched.spawn(entry, TaskMode::Kernel, mapper, frame_allocator);
        TaskHandle { id }
    })
}

/// Return the number of registered tasks.
pub fn task_count() -> usize {
    with_scheduler(|sched| sched.tasks.len())
}

/// Request the scheduler to make `task` the next runnable slot.
pub fn select_task(task: TaskId) -> bool {
    with_scheduler(|sched| {
        if task < sched.tasks.len() && sched.tasks[task].is_some() {
            sched.current = (task + sched.tasks.len() - 1) % sched.tasks.len();
            true
        } else {
            false
        }
    })
}

/// Yield execution to the scheduler.
pub fn yield_now() {
    unsafe { yield_now_raw() }
}

/// Enter the first scheduled task.
pub fn start() -> ! {
    crate::task::scheduler::start_first()
}

unsafe extern "C" {
    #[link_name = "yield_now"]
    fn yield_now_raw();
}
