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
    arch::x86_64::gdt::set_kernel_stack,
    arch::x86_64::interrupts::end_of_interrupt,
    graph::{BundleId, KERNEL_BUNDLE_ID},
    serial_print,
};
use alloc::vec::Vec;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};
use log::info;
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, Mapper, PhysFrame, Size4KiB, Translate};
use x86_64::{PhysAddr, registers::control::Cr3};

use crate::task::context::{FullContext, TaskMode, prepare_context};

/// Represents a schedulable task with its execution context and stack.
///
/// A Task is a running instance of a Bundle. It has its own stack,
/// CPU context, and identity, but shares the Bundle's code and contract.
///
/// In the current implementation, each Task runs in its own protection domain (process),
/// but future versions may host multiple Tasks in a single process.
#[repr(C)]
#[derive(Debug)]
pub struct Task {
    pub entry_point: extern "C" fn(),
    pub stack_top: u64,
    pub context: FullContext,
    pub initialized: bool,
    pub mode: TaskMode,
    /// The ID of the bundle this task is an instance of.
    pub bundle: BundleId,
    pub cr3: u64,
    pub magic: u64,
}

impl Task {
    const STACK_PAGES: u64 = 64; // 64 pages (256KB)
    const STACK_SIZE: u64 = 4096 * Self::STACK_PAGES;
    const MAGIC: u64 = 0x5441534B5F4D4147;

    pub fn new(
        entry: extern "C" fn(),
        index: usize,
        mode: TaskMode,
        mapper: &mut (impl Mapper<Size4KiB> + Translate),
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Self {
        let mut task = Self {
            entry_point: entry,
            stack_top: 0,
            context: unsafe { core::mem::zeroed() },
            initialized: false,
            mode,
            bundle: KERNEL_BUNDLE_ID,
            cr3: Cr3::read().0.start_address().as_u64(),
            magic: Self::MAGIC,
        };

        task.allocate_stack_if_needed(mapper, frame_allocator, index);
        task.prepare_if_needed();
        info!(
            "Task {} created: entry={:#x} stack_top={:#x} mode={:?}",
            index, task.entry_point as u64, task.stack_top, mode
        );
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
        mapper: &mut (impl Mapper<Size4KiB> + Translate),
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        index: usize,
    ) {
        if self.stack_top == 0 {
            use x86_64::VirtAddr;
            use x86_64::structures::paging::{Page, PageTableFlags};

            let base_virt = Self::stack_base_for_task(index);
            let start = VirtAddr::new(base_virt);
            let mut page = Page::containing_address(start);

            for i in 0..Self::STACK_PAGES {
                let frame = frame_allocator
                    .allocate_frame()
                    .expect("Out of physical frames for task stack");

                if i == 0 || i == Self::STACK_PAGES - 1 {
                    info!(
                        "Mapping stack page {} at {:?} to {:?}",
                        i,
                        page.start_address(),
                        frame.start_address()
                    );
                }

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

                    // Verify mapping by writing to the start of the page
                    let ptr = page.start_address().as_mut_ptr::<u64>();
                    ptr.write_volatile(0xCAFEBABE);
                }

                page = page + 1;
            }

            // Reserve space at the top of the stack.
            // We subtract 128 bytes for red zone / safety.
            // We also subtract 8 bytes to ensure 16-byte alignment for the entry point.
            // The x86_64 System V ABI requires (rsp + 8) to be 16-byte aligned on function entry.
            // Since we jump directly via iretq, rsp will be exactly stack_top.
            // So we want stack_top % 16 == 8.
            self.stack_top = base_virt + Self::STACK_SIZE - 128 - 8;
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

const BTREE_WATCH_FN_START: u64 = 0xffffffff8004c830;
const BTREE_WATCH_FN_END: u64 = 0xffffffff8004c900;

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
        mapper: &mut (impl Mapper<Size4KiB> + Translate),
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
        let task = Task::new(entry, self.tasks.len(), mode, mapper, frame_allocator);
        info!("Task {} spawned", self.tasks.len());
        self.tasks.push(Some(task));
    }

    pub fn next_ready_task(&mut self, _now: u64) -> Option<&mut Task> {
        if SCHED_SINGLE_TASK.load(Ordering::Relaxed) {
            if !self.tasks.is_empty() {
                if let Some(ref mut task) = self.tasks[0] {
                    self.current = 0;
                    return Some(task);
                }
            }
            return None;
        }

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
            info!("Task addr: {:p}, Context addr: {:p}", task, &task.context);
            serial_print!("]");
            task.context_ptr()
        } else {
            panic!("No task in slot 0 to start");
        }
    }
}

pub fn start_first() -> ! {
    unsafe extern "C" {
        fn restore_context(saved: *const FullContext) -> !;
    }

    info!("Starting first task");
    let (ctx_ptr, stack_top) = {
        let scheduler = SCHEDULER.lock();
        if let Some(task) = scheduler.tasks[0].as_ref() {
            unsafe {
                CURRENT_TASK = scheduler.tasks[0].as_ref().unwrap() as *const Task as *mut Task;
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
            (&task.context as *const FullContext, task.stack_top)
        } else {
            panic!("No task in slot 0 to start");
        }
    };

    // Ensure the kernel stack is set for the first task
    set_kernel_stack(stack_top);

    unsafe {
        let rip = (*ctx_ptr).frame.rip;
        let is_canonical = |v: u64| {
            let sign = v >> 47;
            sign == 0 || sign == 0x1ffff
        };
        assert!(
            is_canonical(rip),
            "Non-canonical RIP in start_first: {:#x}",
            rip
        );
        if (*ctx_ptr).frame.cs == 0x8 {
            assert!(
                rip >= 0xffffffff80000000 && rip < 0xffffffff90000000,
                "RIP out of kernel text region in start_first: {:#x}",
                rip
            );
        }

        restore_context(ctx_ptr)
    }
}

/// Global scheduler instance.
/// WARNING: Do not use full logging macros from inside the scheduler lock or critical paths.
/// Use klog_irq! only.
pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new(crate::clock::ticks_since_boot));
#[unsafe(no_mangle)]
pub static mut CURRENT_TASK: *mut Task = core::ptr::null_mut();

pub static SCHED_SINGLE_TASK: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn restore_context(ctx: *const FullContext) -> !;
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_schedule_and_switch(current_rsp: *const u8, irq: u8) -> ! {
    // WARNING: This function runs in interrupt context.
    // Do not use full logging macros (info!, kinfo!, etc.) here; use klog_irq! only.

    if (current_rsp as u64) % 8 != 0 {
        panic!("Unaligned RSP: {:p}", current_rsp);
    }

    unsafe {
        if !CURRENT_TASK.is_null() {
            let task = &mut *CURRENT_TASK;
            if task.magic != Task::MAGIC {
                panic!("Current task magic corrupted: {:#x}", task.magic);
            }
            let saved_cs_offset = (15 + 1) * core::mem::size_of::<u64>(); // index 16: after regs + RIP
            let saved_cs = *(current_rsp.add(saved_cs_offset) as *const u64);
            let incoming_mode = if saved_cs & 0x3 == 0x3 {
                TaskMode::User
            } else {
                TaskMode::Kernel
            };
            if task.mode != incoming_mode {
                // crate::klog_irq!(b'M');
                task.mode = incoming_mode;
            }

            let words_pushed = 15 + 5; // Always 5 words (RIP, CS, RFLAGS, RSP, SS) + 15 regs
            let context_size = words_pushed * core::mem::size_of::<u64>();
            let dst = task.context_mut_ptr();

            if (dst as u64) % 8 != 0 {
                panic!("Unaligned context ptr: {:p}", dst);
            }

            if current_rsp != dst {
                ptr::copy_nonoverlapping(current_rsp, dst, context_size);
            }

            let saved_ctx = current_rsp as *const FullContext;
            if let Some(ctx) = saved_ctx.as_ref() {
                let rip = ctx.frame.rip;
                if rip >= BTREE_WATCH_FN_START && rip < BTREE_WATCH_FN_END {
                    /*log::error!(
                        "Saving context in btree watch fn: rip={:#x} rsi={:#x} rsp={:#x}",
                        rip,
                        ctx.regs.rsi,
                        ctx.frame.rsp
                    );*/
                    crate::klog_irq!(b'E');
                }
            }

            // crate::klog_irq!(b'S');
        }

        let mut scheduler = SCHEDULER.lock();
        let now = (scheduler.now_fn)();
        let next = scheduler.next_ready_task(now).map(|t| t as *mut Task);
        let current = CURRENT_TASK;
        let current_idx = scheduler.current;
        drop(scheduler);
        match next {
            Some(task_ptr) => {
                if (*task_ptr).magic != Task::MAGIC {
                    panic!("Next task magic corrupted: {:#x}", (*task_ptr).magic);
                }
                CURRENT_TASK = task_ptr;

                end_of_interrupt(irq);
                if (*task_ptr).context.frame.rip >= BTREE_WATCH_FN_START
                    && (*task_ptr).context.frame.rip < BTREE_WATCH_FN_END
                {
                    /*log::error!(
                        "Restoring context in btree watch fn: rip={:#x} rsi={:#x} rdi={:#x} rsp={:#x}",
                        (*task_ptr).context.frame.rip,
                        (*task_ptr).context.regs.rsi,
                        (*task_ptr).context.regs.rdi,
                        (*task_ptr).context.frame.rsp
                    );*/
                    crate::klog_irq!(b'E');
                }
                crate::klog_irq!(b'W');

                // Step 2: Assert canonical RSP
                let rsp = (*task_ptr).context.frame.rsp;
                let is_canonical = rsp < 0x0000_8000_0000_0000 || rsp >= 0xFFFF_8000_0000_0000;
                if !is_canonical {
                    panic!("Non-canonical RSP detected before switch: {:#x}", rsp);
                }

                crate::trace::trace_event(
                    crate::trace::TraceKind::SwitchTo,
                    current_idx as u16,
                    (*task_ptr).cr3,
                );

                set_kernel_stack((*task_ptr).stack_top);

                // Switch CR3
                let new_cr3 = PhysFrame::containing_address(PhysAddr::new((*task_ptr).cr3));
                let current_cr3 = Cr3::read().0;
                if new_cr3 != current_cr3 {
                    // crate::klog_irq!(b'C');
                    Cr3::write(new_cr3, Cr3::read().1);
                    let actual_cr3 = Cr3::read().0;
                    if actual_cr3 != new_cr3 {
                        panic!(
                            "CR3 switch failed: expected {:#x}, got {:#x}",
                            new_cr3.start_address().as_u64(),
                            actual_cr3.start_address().as_u64()
                        );
                    }
                }

                // Validate RIP before restoring
                let rip = (*task_ptr).context.frame.rip;
                let is_canonical = |v: u64| {
                    let sign = v >> 47;
                    sign == 0 || sign == 0x1ffff
                };
                if !is_canonical(rip) {
                    panic!("Non-canonical RIP in scheduler (next task): {:#x}", rip);
                }
                if (*task_ptr).context.frame.cs == 0x8 {
                    if rip < 0xffffffff80000000 || rip >= 0xffffffff90000000 {
                        panic!(
                            "RIP out of kernel text region in scheduler (next task): {:#x}",
                            rip
                        );
                    }
                }

                restore_context(&(*task_ptr).context)
            }
            None => {
                end_of_interrupt(irq);
                let ctx_ptr = if !current.is_null() {
                    &(*current).context as *const FullContext
                } else {
                    // error!("No current task; esperante.");
                    crate::klog_irq!(b'N');
                    rust_schedule_and_switch(current_rsp, irq);
                };
                CURRENT_TASK = current;

                // Validate RIP before restoring
                let rip = (*ctx_ptr).frame.rip;
                let is_canonical = |v: u64| {
                    let sign = v >> 47;
                    sign == 0 || sign == 0x1ffff
                };
                if !is_canonical(rip) {
                    panic!("Non-canonical RIP in scheduler: {:#x}", rip);
                }
                if (*ctx_ptr).frame.cs == 0x8 {
                    if rip < 0xffffffff80000000 || rip >= 0xffffffff90000000 {
                        panic!("RIP out of kernel text region in scheduler: {:#x}", rip);
                    }
                }

                restore_context(ctx_ptr)
            }
        }
    }
}
