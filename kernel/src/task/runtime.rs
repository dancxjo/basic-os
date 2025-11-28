use crate::graph::{BundleId, BundleType, KERNEL_BUNDLE_ID};
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

/// Spawn a new kernel-mode task with an associated bundle.
/// This creates the bundle node in the graph and assigns it to the task before first run.
pub fn spawn_kernel_with_bundle(
    entry: extern "C" fn(),
    bundle_name: &str,
    bundle_type: BundleType,
) -> (TaskHandle, BundleId) {
    use crate::graph;

    // Create or get the bundle
    let bundle_id = graph::create_bundle(bundle_name, bundle_type, None);

    // Spawn the task
    let handle = spawn_kernel(entry);

    // Assign the bundle to the task before first run
    assign_bundle(handle.id(), bundle_id);

    (handle, bundle_id)
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

#[cfg(debug_assertions)]
pub fn debug_dump_current_task() {
    unsafe {
        let task_ptr = crate::task::scheduler::CURRENT_TASK;
        if !task_ptr.is_null() {
            let task = &*task_ptr;
            log::info!(
                "Current Task: mode={:?}, bundle={}, rip={:#x}, rsp={:#x}",
                task.mode,
                task.bundle,
                task.context.frame.rip,
                task.context.frame.rsp
            );
        } else {
            log::info!("Current Task: NULL");
        }
    }
}
