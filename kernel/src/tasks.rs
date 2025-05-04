//! tasks.rs — Tiny preemptive task manager for ThingOS

use core::mem::MaybeUninit;

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
                    let ptr = (next.stack_pointer as *const u64).sub(18 - i);
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
        log::info!("SP[{}] = {:#018x}", i, unsafe { *sp.offset(-(17 - i)) });
    }
    log::info!("Calling switch_to_task with RSP = {:p}", first);
    unsafe {
        switch_to_task(first); // Assembly function that restores context and iretqs
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn schedule(old_rsp: *mut usize) -> *mut usize {
    log::info!("Scheduling...");
    const CONTEXT_SIZE: isize = 15;

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

            CURRENT_TASK = next_task;
            log::info!(
                "Switching to task {}, RSP = {:p} (saved old RSP was {:p})",
                next_task,
                next_rsp,
                old_rsp
            );

            end_of_interrupt(0);
            return next_rsp as *mut usize;
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

    let aligned_rsp = unsafe {
        let start = raw_top.sub(TOTAL_ENTRIES * 8);
        assert_eq!(start as usize % 16, 0, "Initial stack not 16-byte aligned!");
        let mut ptr = start as *mut u64;

        // GPRs in rollback pop order (will be popped *after* switch to task stack)
        for i in 0..NUM_GPR {
            *ptr.add(i) = 0xdeadbeefdeadbee0u64 - i as u64;
        }

        // iret frame (to match order expected by iretq)
        *ptr.add(NUM_GPR + 0) = entry as usize as u64; // RIP
        *ptr.add(NUM_GPR + 1) = 0x08; // CS
        *ptr.add(NUM_GPR + 2) = 0x202; // RFLAGS (IF=1)

        ptr.add(NUM_GPR + 3) as *mut u8
    };

    log::info!("Fabricated task stack at {:p} (aligned)", aligned_rsp);

    Task {
        stack_pointer: aligned_rsp,
        _stack: stack,
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(align(16))]
struct Aligned(u8);

fn allocate_stack() -> (Box<[u8]>, *mut u8) {
    let mut stack: Box<[Aligned]> = vec![Aligned(0); STACK_SIZE].into_boxed_slice();
    let stack_ptr = stack.as_mut_ptr() as *mut u8;
    let stack_top = unsafe { stack_ptr.add(STACK_SIZE) };
    log::info!("Allocated stack from {:p} to {:p}", stack_ptr, stack_top);

    (
        unsafe {
            Box::from_raw(core::slice::from_raw_parts_mut(
                stack.as_mut_ptr() as *mut u8,
                STACK_SIZE,
            ))
        },
        stack_top,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn check_alignment(rsp: usize) {
    info!("Is this aligned? RSP = {:#018x}", rsp);
}

#[repr(C)]
pub struct InterruptContext {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    // rsp, ss would follow if we returned to user space
}

#[unsafe(no_mangle)]
pub extern "C" fn spy_context(ptr: *const InterruptContext) {
    let ctx = unsafe { &*ptr };
    serial_println!("=== INTERRUPT CONTEXT DUMP ===");
    serial_println!(" RIP: {:016x}", ctx.rip);
    serial_println!(" CS : {:016x}", ctx.cs);
    serial_println!(" RFLAGS: {:016x}", ctx.rflags);
    serial_println!(" RAX: {:016x}", ctx.rax);
    serial_println!(" RBX: {:016x}", ctx.rbx);
    serial_println!(" RCX: {:016x}", ctx.rcx);
    serial_println!(" RDX: {:016x}", ctx.rdx);
    serial_println!(" RSI: {:016x}", ctx.rsi);
    serial_println!(" RDI: {:016x}", ctx.rdi);
    serial_println!(" RBP: {:016x}", ctx.rbp);
    // serial_println!(" RSP: {:016x}", ctx.rsp);
    serial_println!(" R8 : {:016x}", ctx.r8);
    serial_println!(" R9 : {:016x}", ctx.r9);
    serial_println!(" R10: {:016x}", ctx.r10);
    serial_println!(" R11: {:016x}", ctx.r11);
    serial_println!(" R12: {:016x}", ctx.r12);
    serial_println!(" R13: {:016x}", ctx.r13);
    serial_println!(" R14: {:016x}", ctx.r14);
    serial_println!(" R15: {:016x}", ctx.r15);
}

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

// Replace with your actual allocator
extern crate alloc;
use alloc::alloc::alloc;

#[unsafe(no_mangle)]
pub extern "C" fn alloc_stack(size: usize) -> *mut u8 {
    // Ensure alignment to 16 bytes
    let layout = Layout::from_size_align(size, 16).unwrap();

    // Allocate from the global allocator
    let ptr = unsafe { alloc(layout) };

    if ptr.is_null() {
        panic!("alloc_stack: failed to allocate stack of {} bytes", size);
    }

    ptr
}

#[unsafe(no_mangle)]
pub fn prepare_task_stack(entry_point: extern "C" fn() -> !) -> *mut u8 {
    const NUM_GPRS: usize = 15;
    const STACK_SIZE: usize = 4096;

    let layout = Layout::from_size_align(STACK_SIZE, 16).unwrap();
    let raw = unsafe { alloc::alloc::alloc(layout) };
    assert!(!raw.is_null());

    let stack_top = unsafe { raw.add(STACK_SIZE) };
    let mut sp = stack_top as *mut u64;
    unsafe {
        // Push RFLAGS
        sp = unsafe { sp.offset(-1) };
        *sp = 0x10282;

        // Push CS
        sp = unsafe { sp.offset(-1) };
        *sp = 0x08;

        // Push RIP
        sp = unsafe { sp.offset(-1) };
        *sp = entry_point as u64;

        let regs = [
            0x0000000000000000, // r15
            0x0000000000000000, // r14
            0x0000000000000000, // r13
            0x0000000000000000, // r12
            0x0000000000000010, // r11
            0x0000000000000002, // r10
            0x0000000000000001, // r9
            0x0000000000000000, // r8
            0x0000000000000000, // rsi
            0xffffffff81fda8d0, // rdi
            0x0000000000000000, // rbp
            0x0000000000000001, // rdx
            0x0000000000000000, // rcx
            0x0000000000000000, // rbx
            0xffff80007fe1ca20, // rax
        ];

        for &val in regs.iter().rev() {
            sp = unsafe { sp.offset(-1) };
            *sp = val;
        }
    }
    let context_base = sp; // stack[0]
    let iret_rsp = unsafe { sp.add(NUM_GPRS) }; // stack[15]

    // Print the stack (optional)
    for i in 0..(NUM_GPRS + 3) {
        serial_println!("FabricatedStack[{}] = 0x{:016x}", i, unsafe {
            *context_base.add(i)
        });
    }

    iret_rsp as *mut u8 // return pointer to RIP slot (stack[15])
}

#[unsafe(no_mangle)]
pub extern "C" fn dummy_task() -> ! {
    serial_println!("Dummy task entered!");
    loop {
        x86_64::instructions::hlt();
    }
}
