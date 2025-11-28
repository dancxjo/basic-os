use crate::mm::allocator::{BootFrameAllocator, global_mapper};
use crate::task::context::TaskMode;
use crate::task::scheduler::{SCHEDULER, Scheduler};
use crate::telemetry::graph::{BundleId, KERNEL_BUNDLE_ID};

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

/// Mark the given task as belonging to a bundle.
pub fn assign_bundle(task: TaskId, bundle: BundleId) {
    with_scheduler(|sched| {
        if let Some(Some(t)) = sched.tasks.get_mut(task).map(|t| t.as_mut()) {
            t.bundle = bundle;
        }
    });
}

/// Mark the current task as belonging to a bundle.
pub fn assign_current_bundle(bundle: BundleId) {
    unsafe {
        let task_ptr = crate::task::scheduler::CURRENT_TASK;
        if !task_ptr.is_null() {
            (*task_ptr).bundle = bundle;
        }
    }
}

/// Return the bundle associated with the currently running task.
pub fn current_bundle() -> BundleId {
    unsafe {
        let task_ptr = crate::task::scheduler::CURRENT_TASK;
        if !task_ptr.is_null() {
            return (*task_ptr).bundle;
        }
    }
    KERNEL_BUNDLE_ID
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
