//! Task scheduler implementation.
//!
//! The scheduler manages a set of tasks and switches between them using
//! timer interrupts. The main entry point for context switching is
//! `rust_schedule_and_switch`, which is called from the assembly
//! tick_handler when a timer interrupt occurs.
//!
//! # Architecture
//!
//! 1. Timer interrupt fires, tick_handler.S saves all general-purpose registers
//! 2. rust_schedule_and_switch copies saved context to current task
//! 3. Scheduler selects next task using round-robin
//! 4. restore_context.S restores the next task's registers and iretq's to it
//!
//! # Memory Layout
//!
//! Each task has its own stack at a unique virtual address:
//! - Stack region base: 0xffff_8800_1000_0000
//! - Stack size: 5 pages (20KB)
//! - Tasks are spaced by stack size

use crate::{
    arch::x86_64::gdt::SELECTORS,
    arch::x86_64::interrupts::end_of_interrupt,
    graph::{BundleId, KERNEL_BUNDLE_ID},
    mm::allocator::BootFrameAllocator,
    serial_print,
};
use alloc::vec::Vec;
use core::ptr;
use log::{error, info};
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, Mapper, OffsetPageTable};

use crate::task::context::{FullContext, TaskMode, prepare_context};

/// Represents a schedulable task with its execution context and stack.
#[repr(C)]
#[derive(Debug)]
pub struct Task {
    pub entry_point: extern "C" fn(),
    pub stack_top: u64,
    pub context: FullContext,
    pub initialized: bool,
    pub mode: TaskMode,
    pub bundle: BundleId,
}

impl Task {
    const STACK_PAGES: u64 = 16;
    const STACK_SIZE: u64 = 4096 * Self::STACK_PAGES;

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
            bundle: KERNEL_BUNDLE_ID,
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
        STACK_REGION_BASE + index as u64 * Self::STACK_SIZE
    }

    pub const fn stack_size() -> u64 {
        Self::STACK_SIZE
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

            for _ in 0..Self::STACK_PAGES {
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

            self.stack_top = base_virt + Self::STACK_SIZE - 128;
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

    pub fn first_task_context(&self) -> *const u8 {
        if let Some(task) = self.tasks[0].as_ref() {
            unsafe {
                CURRENT_TASK = self.tasks[0].as_ref().unwrap() as *const Task as *mut Task;
            }
            info!(
                "First task context: rip={:#x} cs={:#x} rsp={:#x} ss={:#x} mode={:?}",
                task.context.frame.rip,
                task.context.frame.cs,
                task.context.frame.rsp,
                task.context.frame.ss,
                task.mode
            );
            serial_print!("]");
            task.context_ptr()
        } else {
            panic!("No task in slot 0 to start");
        }
    }
}

pub fn start_first() -> ! {
    unsafe extern "C" {
        fn restore_context(saved: *const u8) -> !;
    }

    info!("Starting first task");
    let ctx = {
        let scheduler = SCHEDULER.lock();
        scheduler.first_task_context()
    };
    unsafe { restore_context(ctx) }
}

pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new(crate::clock::ticks_since_boot));
#[unsafe(no_mangle)]
pub static mut CURRENT_TASK: *mut Task = core::ptr::null_mut();

unsafe extern "C" {
    fn restore_context(ctx: *const u8) -> !;
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_schedule_and_switch(current_rsp: *const u8, irq: u8) -> ! {
    serial_print!("S");

    unsafe {
        if !CURRENT_TASK.is_null() {
            let task = &mut *CURRENT_TASK;

            let saved_cs_offset = (15 + 1) * core::mem::size_of::<u64>(); // index 16: after regs + RIP
            let saved_cs = *(current_rsp.add(saved_cs_offset) as *const u64);
            let incoming_mode = if saved_cs & 0x3 == 0x3 {
                TaskMode::User
            } else {
                TaskMode::Kernel
            };
            if task.mode != incoming_mode {
                info!(
                    "Task mode transition: {:?} -> {:?} (irq={}, cs={:#x}, rsp={:#x})",
                    task.mode, incoming_mode, irq, saved_cs, current_rsp as u64
                );
                task.mode = incoming_mode;
            }

            let words_pushed = if incoming_mode == TaskMode::Kernel {
                // Kernel: CPU pushed RIP/CS/RFLAGS (no SS/RSP)
                15 + 3
            } else {
                // User: CPU pushed RIP/CS/RFLAGS/RSP/SS
                15 + 5
            };
            let context_size = words_pushed * core::mem::size_of::<u64>();
            let dst = task.context_mut_ptr();

            if current_rsp != dst {
                ptr::copy_nonoverlapping(current_rsp, dst, context_size);
            }

            // For kernel-mode contexts, the CPU doesn't push SS/RSP on interrupt entry.
            // Reconstruct them so the saved context can restore the proper stack pointer.
            let saved = dst as *mut FullContext;
            if (*saved).frame.cs & 0x3 == 0 {
                #[allow(static_mut_refs)]
                let selectors = SELECTORS.as_ref().expect("GDT not initialized");
                (*saved).frame.ss = (selectors.data.0 & !0x3) as u64;
                // Original RSP before the interrupt: 15 registers + RIP/CS/RFLAGS.
                (*saved).frame.rsp = current_rsp.add((15 + 3) * core::mem::size_of::<u64>()) as u64;
            }
        }

        let mut scheduler = SCHEDULER.lock();
        let now = (scheduler.now_fn)();
        let next = scheduler.next_ready_task(now).map(|t| t as *mut Task);
        let current = CURRENT_TASK;
        drop(scheduler);
        match next {
            Some(task_ptr) => {
                CURRENT_TASK = task_ptr;

                end_of_interrupt(irq);
                let next_mode = if (*task_ptr).context.frame.cs & 0x3 == 0x3 {
                    TaskMode::User
                } else {
                    TaskMode::Kernel
                };
                info!(
                    "Switching to {:?} task: rip={:#x}, cs={:#x}, rsp={:#x}, ss={:#x}",
                    next_mode,
                    (*task_ptr).context.frame.rip,
                    (*task_ptr).context.frame.cs,
                    (*task_ptr).context.frame.rsp,
                    (*task_ptr).context.frame.ss
                );
                serial_print!("[{:p}:{:p}]> ", task_ptr, (*task_ptr).context_ptr());
                restore_context((*task_ptr).context_ptr())
            }
            None => {
                serial_print!("!");

                end_of_interrupt(irq);
                let ctx = if !current.is_null() {
                    (*current).context_ptr()
                } else {
                    error!("No current task; esperante.");
                    rust_schedule_and_switch(current_rsp, irq);
                };
                CURRENT_TASK = current;
                restore_context(ctx)
            }
        }
    }
}
