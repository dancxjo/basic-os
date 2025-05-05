use crate::interrupts::end_of_interrupt;
use core::ptr;
use log::info;
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, Mapper};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IretFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GeneralRegisters {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct Task {
    pub entry_point: extern "C" fn(),
    pub stack_top: u64,
    pub saved_regs: GeneralRegisters,
    pub saved_frame: IretFrame,
    pub initialized: bool,
}

impl Task {
    pub fn new(
        entry: extern "C" fn(),
        index: usize,
        mapper: &mut x86_64::structures::paging::OffsetPageTable,
        frame_allocator: &mut crate::allocator::BootFrameAllocator,
    ) -> Self {
        let mut task = Self {
            entry_point: entry,
            stack_top: 0,
            saved_regs: GeneralRegisters {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: 0,
                r11: 0,
                r10: 0,
                r9: 0,
                r8: 0,
                rsi: 0,
                rdi: 0,
                rbp: 0,
                rdx: 0,
                rcx: 0,
                rbx: 0,
                rax: 0,
            },
            saved_frame: IretFrame {
                rip: 0,
                cs: 0,
                rflags: 0,
                rsp: 0,
                ss: 0,
            },
            initialized: false,
        };

        task.allocate_stack_if_needed(mapper, frame_allocator, index);
        task.prepare_if_needed();
        task
    }

    pub fn prepare_if_needed(&mut self) {
        if !self.initialized {
            self.saved_frame = IretFrame {
                rip: self.entry_point as u64,
                cs: 0x08,
                rflags: 0x202,
                rsp: self.stack_top,
                ss: 0x10,
            };
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
        mapper: &mut x86_64::structures::paging::OffsetPageTable,
        frame_allocator: &mut crate::allocator::BootFrameAllocator,
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
        &self.saved_regs as *const _ as *const u8
    }

    pub fn context_mut_ptr(&mut self) -> *mut u8 {
        &mut self.saved_regs as *mut _ as *mut u8
    }
}

pub struct Scheduler {
    pub tasks: [Option<Task>; 4],
    pub current: usize,
    pub last_switched_at: u64,
    pub now_fn: fn() -> u64,
}

impl Scheduler {
    pub const fn new(now_fn: fn() -> u64) -> Self {
        Scheduler {
            tasks: [None, None, None, None],
            current: 0,
            last_switched_at: 0,
            now_fn,
        }
    }

    pub fn spawn(
        &mut self,
        entry: extern "C" fn(),
        index: usize,
        mapper: &mut x86_64::structures::paging::OffsetPageTable,
        frame_allocator: &mut crate::allocator::BootFrameAllocator,
    ) {
        let task = Task::new(entry, index, mapper, frame_allocator);
        info!("Task {} spawned", index);
        self.tasks[index] = Some(task);
    }

    pub fn next_ready_task(&mut self, now: u64) -> Option<&mut Task> {
        let delta = now - self.last_switched_at;
        if delta < 12500 {
            // info!("Skipping switch ({} ticks too soon)", 2500 - delta);
            return None;
        }

        for i in 0..self.tasks.len() {
            let index = (self.current + i) % self.tasks.len();
            if let Some(ref mut task) = self.tasks[index] {
                self.current = index;
                self.last_switched_at = now; // ✅ Move it here!
                return Some(task);
            }
        }

        None
    }

    pub fn start_first(&self) -> ! {
        unsafe extern "C" {
            fn restore_context(saved: *const u8) -> !;
        }
        info!("Starting first task");
        if let Some(task) = self.tasks[0].as_ref() {
            let ctx = task.context_ptr();
            unsafe {
                restore_context(ctx);
            }
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
    use core::ptr;

    unsafe {
        if !CURRENT_TASK.is_null() {
            let task = &mut *CURRENT_TASK;
            ptr::copy_nonoverlapping(
                current_rsp,
                task.context_mut_ptr(),
                core::mem::size_of::<GeneralRegisters>(),
            );
        }

        let mut scheduler = SCHEDULER.lock();
        let now = (scheduler.now_fn)();
        let next = scheduler.next_ready_task(now).map(|t| t as *mut Task);
        let current = CURRENT_TASK;
        drop(scheduler);

        match next {
            Some(task_ptr) => {
                CURRENT_TASK = task_ptr;
                end_of_interrupt(0);
                restore_context((*task_ptr).context_ptr());
            }
            None => {
                end_of_interrupt(0);
                // info!("No switch occurred, returning to current task.");
                restore_context((*current).context_ptr());
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_prepare_task_if_needed(task: &mut Task) {
    task.prepare_if_needed();
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_get_context_ptr(task: &Task) -> *const u8 {
    task.context_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn save_current_context(ctx_ptr: *const u8) {
    unsafe {
        let regs_ptr = ctx_ptr as *const GeneralRegisters;
        let frame_ptr = ctx_ptr.add(core::mem::size_of::<GeneralRegisters>()) as *const IretFrame;

        let task = &mut *CURRENT_TASK;
        ptr::copy_nonoverlapping(regs_ptr, &mut task.saved_regs, 1);
        ptr::copy_nonoverlapping(frame_ptr, &mut task.saved_frame, 1);
    }
}
