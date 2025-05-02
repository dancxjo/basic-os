//! tasks.rs — Tiny preemptive task manager for ThingOS

use core::mem::MaybeUninit;
use core::ptr::null_mut;

use alloc::boxed::Box;
use alloc::vec;
use log::info;

use crate::interrupts::end_of_interrupt;

pub struct Task {
    pub(crate) stack_pointer: *mut u8,
    pub(crate) _stack: Box<[u8]>, // Keep the memory alive
}

const MAX_TASKS: usize = 8;
const STACK_SIZE: usize = 4096 * 4; // 16 KiB
static mut TASKS: MaybeUninit<[Option<Task>; MAX_TASKS]> = MaybeUninit::uninit();

static mut CURRENT_TASK: usize = 0;

pub fn init_tasks() {
    unsafe {
        #[allow(static_mut_refs)]
        TASKS.write(core::array::from_fn(|_| None));
    }
}

pub fn spawn(entry: extern "C" fn()) {
    unsafe {
        #[allow(static_mut_refs)]
        let tasks = TASKS.assume_init_mut();
        for (i, task) in tasks.iter_mut().enumerate() {
            if task.is_none() {
                let next = init_task(entry);
                log::info!("ALLOCATED STACK: {:p} for task {}", next.stack_pointer, i);
                log::info!("Spawned task {} at entry {:?}", i, entry as *const ());
                *task = Some(next);
                return;
            }
        }
    };
    panic!("Too many tasks — maximum of {}", MAX_TASKS);
}

unsafe extern "C" {
    pub fn switch_to_task(rsp: *const u8) -> !;
}

pub fn kickstart() {
    let first = unsafe {
        #[allow(static_mut_refs)]
        TASKS.assume_init_ref()[0]
            .as_ref()
            .expect("Task 0 not initialized")
            .stack_pointer
    };
    log::info!("Jumping to first task at stack {:p}", first);
    #[allow(static_mut_refs)]
    let sp = unsafe {
        TASKS.assume_init_ref()[0]
            .as_ref()
            .expect("Task 0 not initialized")
            .stack_pointer as *const u64
    };
    for i in 0..16 {
        log::info!("SP[{}] = {:#018x}", i, unsafe { *sp.offset(i) });
    }
    unsafe {
        switch_to_task(first); // Assembly function that restores context and iretqs
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn schedule(old_rsp: *mut usize) -> *mut usize {
    const CONTEXT_SIZE: isize = 15; // 15 registers pushed by tick_handler

    unsafe {
        let current = CURRENT_TASK;

        if let Some(next_task) = find_next_task() {
            #[allow(static_mut_refs)]
            let next_rsp = TASKS.assume_init_ref()[next_task]
                .as_ref()
                .expect("Task not found")
                .stack_pointer;
            if next_rsp.is_null() {
                log::warn!("Next task {} has null stack pointer!", next_task);
                end_of_interrupt(0);
                return old_rsp;
            }
            let rsp = (next_rsp as usize & !0xF) as *mut u8;
            assert_eq!(rsp as usize % 16, 0, "RSP must be 16-byte aligned");

            info!("next rsp is {:?} adjusted to {:?}", next_rsp, rsp);
            assert_eq!(rsp as usize % 16, 0, "Initial stack not 16-byte aligned!");

            CURRENT_TASK = next_task;

            log::info!(
                "Switching to task {}, RSP = {:p} adjusted to = {:p} (saved old RSP was {:p})",
                next_task,
                next_rsp,
                rsp,
                old_rsp
            );

            end_of_interrupt(0);
            return rsp as *mut usize;
        }

        log::warn!("No runnable tasks found. Staying on current task.");
        end_of_interrupt(0);
        old_rsp
    }
}

fn find_next_task() -> Option<usize> {
    unsafe {
        let mut next = CURRENT_TASK;
        #[allow(static_mut_refs)]
        let tasks = TASKS.assume_init_ref();
        for _ in 0..MAX_TASKS {
            next = (next + 1) % MAX_TASKS;
            if tasks[next]
                .as_ref()
                .map_or(false, |task| !task.stack_pointer.is_null())
            {
                return Some(next);
            }
        }
        None
    }
}

pub fn init_task(entry: extern "C" fn()) -> Task {
    const NUM_GP_REGS: usize = 0;
    const IRET_FRAME_SIZE: usize = 3; // RIP, CS, RFLAGS
    const TOTAL_ENTRIES: usize = NUM_GP_REGS + IRET_FRAME_SIZE;

    let (mut stack, raw_top) = allocate_stack();

    let tentative_rsp = unsafe { (raw_top as *mut usize).sub(TOTAL_ENTRIES) };
    let aligned_rsp = (tentative_rsp as usize & !0xF) as *mut usize;

    unsafe {
        *aligned_rsp.add(NUM_GP_REGS + 0) = entry as usize; // RIP
        *aligned_rsp.add(NUM_GP_REGS + 1) = 0x08; // CS
        *aligned_rsp.add(NUM_GP_REGS + 2) = 0x202; // RFLAGS

        for i in 0..NUM_GP_REGS {
            *aligned_rsp.add(i) = 0xDEADBEEFDEADBEEF;
        }
    }

    log::info!("Stack pointer for new task: {:#018x}", aligned_rsp as usize);

    Task {
        stack_pointer: aligned_rsp as *mut u8,
        _stack: stack,
    }
}

fn allocate_stack() -> (Box<[u8]>, *mut u8) {
    let mut stack: Box<[u8]> = vec![0u8; STACK_SIZE].into_boxed_slice();
    let stack_ptr = stack.as_mut_ptr();
    let stack_top = unsafe { stack_ptr.add(STACK_SIZE) };
    (stack, stack_top)
}

#[unsafe(no_mangle)]
pub extern "C" fn check_alignment(rsp: usize) {
    info!("Is this aligned? RSP = {:#018x}", rsp);
    //assert_eq!(rsp % 16, 0, "RSP is not 16-byte aligned!");
}

#[unsafe(no_mangle)]
pub extern "C" fn log_stack_frame(frame: *const u64) {
    // SAFETY: We trust the assembly caller to pass a valid stack frame
    unsafe {
        let regs = core::slice::from_raw_parts(frame, 15);

        log::info!("--- Stack Frame @ {:p} ---", frame);
        log::info!("RAX: {:016x}", regs[0]);
        log::info!("RBX: {:016x}", regs[1]);
        log::info!("RCX: {:016x}", regs[2]);
        log::info!("RDX: {:016x}", regs[3]);
        log::info!("RBP: {:016x}", regs[4]);
        log::info!("RDI: {:016x}", regs[5]);
        log::info!("RSI: {:016x}", regs[6]);
        log::info!("R8 : {:016x}", regs[7]);
        log::info!("R9 : {:016x}", regs[8]);
        log::info!("R10: {:016x}", regs[9]);
        log::info!("R11: {:016x}", regs[10]);
        log::info!("R12: {:016x}", regs[11]);
        log::info!("R13: {:016x}", regs[12]);
        log::info!("R14: {:016x}", regs[13]);
        log::info!("R15: {:016x}", regs[14]);
    }
}
