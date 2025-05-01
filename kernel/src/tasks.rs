//! tasks.rs — Tiny preemptive task manager for ThingOS

use core::ptr::null_mut;

use log::info;

use crate::interrupts::end_of_interrupt;

#[derive(Copy, Clone)]
pub struct Task {
    pub(crate) stack_pointer: *mut u8,
    // Optional: task ID, name, state
}

const MAX_TASKS: usize = 8;
const STACK_SIZE: usize = 4096 * 4; // 16 KiB

static mut TASKS: [Task; MAX_TASKS] = [Task {
    stack_pointer: null_mut(),
}; MAX_TASKS];

static mut CURRENT_TASK: usize = 0;

pub fn spawn(entry: extern "C" fn() -> !) {
    unsafe {
        #[allow(static_mut_refs)]
        for (i, task) in TASKS.iter().enumerate() {
            if task.stack_pointer.is_null() {
                let next = init_task(entry);
                TASKS[i] = next;
                log::info!("ALLOCATED STACK: {:p} for task {}", next.stack_pointer, i);

                log::info!("Spawned task {} at entry {:?}", i, entry as *const ());
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
    let first = unsafe { TASKS[0].stack_pointer };
    log::info!("Jumping to first task at stack {:p}", first);
    unsafe {
        switch_to_task(first); // Assembly function that restores context and iretqs
    }
}

static mut ENTERED_SCHEDULER: bool = false;

#[unsafe(no_mangle)]
pub extern "C" fn schedule(old_rsp: *mut usize) -> *mut usize {
    use crate::interrupts::end_of_interrupt;

    const CONTEXT_SIZE: isize = 15;

    unsafe {
        if !ENTERED_SCHEDULER {
            ENTERED_SCHEDULER = true;
            log::info!("Scheduler entering for the first time.");
        } else {
            let current = CURRENT_TASK;
            let adjusted_old_rsp = old_rsp.offset(-CONTEXT_SIZE);
            TASKS[current].stack_pointer = adjusted_old_rsp as *mut u8;
        }

        if let Some(next_task) = find_next_task() {
            let next_rsp = TASKS[next_task].stack_pointer;
            CURRENT_TASK = next_task;

            // log::info!("Switching to task {}, RSP = {:p}", next_task, next_rsp);

            end_of_interrupt(0);
            return next_rsp as *mut usize;
        }

        log::warn!("No runnable tasks found.");
        end_of_interrupt(0);
        old_rsp
    }
}

pub fn first_stack_pointer() -> *const u8 {
    unsafe { TASKS[0].stack_pointer }
}

fn find_next_task() -> Option<usize> {
    unsafe {
        let mut next = CURRENT_TASK;
        for _ in 0..MAX_TASKS {
            next = (next + 1) % MAX_TASKS;
            if !TASKS[next].stack_pointer.is_null() {
                return Some(next);
            }
        }
        None
    }
}

/// Creates a Task with a fabricated `iretq` frame.
fn init_task(entry: extern "C" fn() -> !) -> Task {
    const CONTEXT_SAVE_SIZE: usize = 15 * 8; // 15 pushed registers
    let stack = allocate_stack();
    let stack_top = unsafe { stack.add(STACK_SIZE) };

    let mut sp = stack_top;

    // Reserve space for what tick_handler will push
    sp = unsafe { sp.offset(-(CONTEXT_SAVE_SIZE as isize)) };

    // Push iretq frame
    unsafe {
        sp = sp.offset(-8);
        *(sp as *mut u64) = 0x202; // RFLAGS (IF = 1)
        sp = sp.offset(-8);
        *(sp as *mut u64) = 0x08; // CS
        sp = sp.offset(-8);
        *(sp as *mut u64) = entry as u64; // RIP
    }

    log::info!("Initializing task at entry {:p}", entry as *const ());
    log::info!("Allocated stack at {:p}", stack);
    log::info!("Stack pointer after iretq frame and context: {:p}", sp);

    Task { stack_pointer: sp }
}

fn allocate_stack() -> *mut u8 {
    use alloc::boxed::Box;

    let boxed: Box<[u8; STACK_SIZE]> = Box::new([0; STACK_SIZE]);
    Box::into_raw(boxed) as *mut u8
}
