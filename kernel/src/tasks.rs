use crate::{
    allocator::BootFrameAllocator,
    interrupts::end_of_interrupt,
    serial_print,
    serial_println,
};
use alloc::vec::Vec;
use core::ptr;
use log::{error, info, trace};
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, Mapper, OffsetPageTable};

use crate::task_context::{prepare_context, FullContext, TaskMode};

#[repr(C)]
#[derive(Debug)]
pub struct Task {
    pub entry_point: extern "C" fn(),
    pub stack_top: u64,
    pub context: FullContext,
    pub initialized: bool,
    pub mode: TaskMode,
}

impl Task {
    pub fn new(
        entry: extern "C" fn(),
        index: usize,
        mode: TaskMode,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut BootFrameAllocator,
    ) -> Self {
        let mut task = Self {
            entry_point: entry,
            stack_top: 0,
            context: unsafe { core::mem::zeroed() },
            initialized: false,
            mode,
        };

        task.allocate_stack_if_needed(mapper, frame_allocator, index);
        task.prepare_if_needed();
        task
    }

    pub fn prepare_if_needed(&mut self) {
        if !self.initialized {
            self.context = prepare_context(self.entry_point, self.stack_top, self.mode);
            self.initialized = true;
        }
        info!("Task initialized");
    }

    pub fn stack_base_for_task(index: usize) -> u64 {
        const STACK_REGION_BASE: u64 = 0xffff_8800_1000_0000;
        const STACK_SIZE: u64 = 4096 * 5;
        STACK_REGION_BASE + index as u64 * STACK_SIZE
    }

    pub fn allocate_stack_if_needed(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut BootFrameAllocator,
        index: usize,
    ) {
        if self.stack_top == 0 {
            use x86_64::VirtAddr;
            use x86_64::structures::paging::{Page, PageTableFlags};

            let base_virt = Self::stack_base_for_task(index);
            let start = VirtAddr::new(base_virt);
            let mut page = Page::containing_address(start);

            for _ in 0..5 {
                let frame = frame_allocator
                    .allocate_frame()
                    .expect("Out of physical frames for task stack");

                unsafe {
                    mapper
                        .map_to(
                            page,
                            frame,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                            frame_allocator,
                        )
                        .expect("map_to failed (task stack)")
                        .flush();
                }

                page = page + 1;
            }

            self.stack_top = base_virt + 5 * 4096;
        }
        info!("Stack allocated for task {}", index);
    }

    pub fn context_ptr(&self) -> *const u8 {
        &self.context as *const _ as *const u8
    }

    pub fn context_mut_ptr(&mut self) -> *mut u8 {
        &mut self.context as *mut _ as *mut u8
    }
}

pub struct Scheduler {
    pub tasks: Vec<Option<Task>>,
    pub current: usize,
    pub last_switched_at: u64,
    pub now_fn: fn() -> u64,
}

impl Scheduler {
    pub const fn new(now_fn: fn() -> u64) -> Self {
        Scheduler {
            tasks: Vec::new(),
            current: 0,
            last_switched_at: 0,
            now_fn,
        }
    }

    pub fn spawn(
        &mut self,
        entry: extern "C" fn(),
        mode: TaskMode,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut BootFrameAllocator,
    ) {
        let task = Task::new(entry, self.tasks.len(), mode, mapper, frame_allocator);
        info!("Task {} spawned", self.tasks.len());
        self.tasks.push(Some(task));
    }

    pub fn next_ready_task(&mut self, _now: u64) -> Option<&mut Task> {
        let index = (self.current + 1) % self.tasks.len();
        serial_print!("\n\r@{}:", index);
        if self.tasks.is_empty() {
            return None;
        }
        if let Some(ref mut task) = self.tasks[index] {
            self.current = index;
            return Some(task);
        }
        None
    }

    pub fn start_first(&self) -> ! {
        unsafe extern "C" {
            fn restore_context(saved: *const u8) -> !;
        }

        info!("Starting first task");
        if let Some(task) = self.tasks[0].as_ref() {
            unsafe { CURRENT_TASK = self.tasks[0].as_ref().unwrap() as *const Task as *mut Task };
            serial_print!("]");
            unsafe { restore_context(task.context_ptr()) };
        } else {
            panic!("No task in slot 0 to start");
        }
    }
}

pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new(crate::clock::ticks_since_boot));
#[unsafe(no_mangle)]
pub static mut CURRENT_TASK: *mut Task = core::ptr::null_mut();

unsafe extern "C" {
    fn restore_context(ctx: *const u8) -> !;
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_schedule_and_switch(current_rsp: *const u8) -> ! {
    serial_print!("'");
    trace!("Scheduling and switching tasks...");

    unsafe {
        if !CURRENT_TASK.is_null() {
            let task = &mut *CURRENT_TASK;
            let context_size = core::mem::size_of::<FullContext>();

            ptr::copy_nonoverlapping(current_rsp, task.context_mut_ptr(), context_size);
        }

        let mut scheduler = SCHEDULER.lock();
        let now = (scheduler.now_fn)();
        let next = scheduler.next_ready_task(now).map(|t| t as *mut Task);
        let current = CURRENT_TASK;
        drop(scheduler);

        match next {
            Some(task_ptr) => {
                CURRENT_TASK = task_ptr;
                log::trace!(
                    "Switching to task at {:p}, ctx = {:p}",
                    task_ptr,
                    (*task_ptr).context_ptr()
                );

                end_of_interrupt(0);
                serial_print!("[{:p}:{:p}]> ", task_ptr, (*task_ptr).context_ptr());
                unsafe { restore_context((*task_ptr).context_ptr()) }
            }
            None => {
                serial_print!("!");

                end_of_interrupt(0);
                let ctx = if !current.is_null() {
                    (*current).context_ptr()
                } else {
                    error!("No current task; esperante.");
                    rust_schedule_and_switch(current_rsp);
                };
                CURRENT_TASK = current;
                unsafe { restore_context(ctx) }
            }
        }
    }
}
