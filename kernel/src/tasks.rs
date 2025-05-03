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
                for (i, val) in (0..18).map(|i| unsafe {
                    let ptr = (next.stack_pointer as *const u64).add(i);
                    (i, *ptr)
                }) {
                    log::info!("FabricatedStack[{}] = {:#018x}", i, val);
                }

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
    for i in 0..18 {
        log::info!("SP[{}] = {:#018x}", i, unsafe { *sp.offset(i) });
    }
    log::info!("Calling switch_to_task with RSP = {:p}", first);
    unsafe {
        switch_to_task(first); // Assembly function that restores context and iretqs
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn schedule(old_rsp: *mut usize) -> *mut usize {
    log::info!("Scheduling...");
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
            let rsp = next_rsp;

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
    const NUM_GPR: usize = 15;
    const IRET_FRAME: usize = 3;
    const TOTAL_ENTRIES: usize = NUM_GPR + IRET_FRAME;

    let (mut stack, raw_top) = allocate_stack();

    let tentative_rsp = unsafe { (raw_top as *mut usize).sub(TOTAL_ENTRIES) };
    // Align to 16 bytes (just in case)
    let aligned_rsp = (tentative_rsp as usize & !0xF) as *mut usize;

    unsafe {
        // Fill registers with sentinels
        for i in 0..NUM_GPR {
            *aligned_rsp.add(i) = 0x1111000000000000 + i as usize;
        }

        // iretq frame (MUST be last 3)
        *aligned_rsp.add(NUM_GPR + 0) = entry as usize; // RIP
        *aligned_rsp.add(NUM_GPR + 1) = 0x08; // CS
        *aligned_rsp.add(NUM_GPR + 2) = 0x202; // RFLAGS
    }

    log::info!("Fabricated task stack at {:p} (aligned)", aligned_rsp);
    for i in 0..(TOTAL_ENTRIES) {
        log::info!("FabricatedStack[{}] = {:#018x}", i, unsafe {
            *aligned_rsp.add(i)
        });
    }

    Task {
        // Stack pointer must point TO the RIP (i.e. skip GPRs)
        stack_pointer: unsafe { aligned_rsp.add(NUM_GPR) } as *mut u8,
        _stack: stack,
    }
}

fn allocate_stack() -> (Box<[u8]>, *mut u8) {
    let mut stack: Box<[u8]> = vec![0u8; STACK_SIZE].into_boxed_slice();
    let stack_ptr = stack.as_mut_ptr();
    let stack_top = unsafe { stack_ptr.add(STACK_SIZE) };
    log::info!("Allocated stack from {:p} to {:p}", stack_ptr, stack_top);

    (stack, stack_top)
}

#[unsafe(no_mangle)]
pub extern "C" fn check_alignment(rsp: usize) {
    info!("Is this aligned? RSP = {:#018x}", rsp);
}

#[unsafe(no_mangle)]
pub extern "C" fn spy_context(context_rsp: *const u64) {
    unsafe {
        let ctx = core::slice::from_raw_parts(context_rsp, 15);
        log::info!("--- Gathered Context ---");
        log::info!("R15: 0x{:016x}", ctx[0]);
        log::info!("R14: 0x{:016x}", ctx[1]);
        log::info!("R13: 0x{:016x}", ctx[2]);
        log::info!("R12: 0x{:016x}", ctx[3]);
        log::info!("R11: 0x{:016x}", ctx[4]);
        log::info!("R10: 0x{:016x}", ctx[5]);
        log::info!(" R9: 0x{:016x}", ctx[6]);
        log::info!(" R8: 0x{:016x}", ctx[7]);
        log::info!("RSI: 0x{:016x}", ctx[8]);
        log::info!("RDI: 0x{:016x}", ctx[9]);
        log::info!("RBP: 0x{:016x}", ctx[10]);
        log::info!("RDX: 0x{:016x}", ctx[11]);
        log::info!("RCX: 0x{:016x}", ctx[12]);
        log::info!("RBX: 0x{:016x}", ctx[13]);
        log::info!("RAX: 0x{:016x}", ctx[14]);
    };
}
