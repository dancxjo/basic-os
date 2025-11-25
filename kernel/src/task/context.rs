//! Task context structures for x86_64.
//!
//! These structures define the saved CPU state for context switching.
//! The layout must match the assembly code in tick_handler.S and restore_context.S.

/// Task execution mode (privilege level).
#[derive(Debug, Clone, Copy)]
pub enum TaskMode {
    Kernel,
    User,
}

/// Interrupt return frame pushed by the CPU on interrupt/exception.
/// This is the state restored by the `iretq` instruction.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IretFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

/// General-purpose registers saved during context switch.
///
/// IMPORTANT: The order of fields MUST match the push order in tick_handler.S
/// and pop order in restore_context.S. When pushq is used in order:
/// rax, rbx, rcx, rdx, rbp, rdi, rsi, r8-r15, the stack layout (low to high)
/// becomes: r15, r14, ..., rax.
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

/// Complete saved context for a task, combining registers and interrupt frame.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FullContext {
    pub regs: GeneralRegisters,
    pub frame: IretFrame,
}

/// Prepare initial context for a new task.
///
/// Creates a context that, when restored, will start executing at `entry`
/// with the stack pointer set to `stack_top`.
pub fn prepare_context(entry: extern "C" fn(), stack_top: u64, mode: TaskMode) -> FullContext {
    use crate::arch::x86_64::gdt::{
        KERNEL_CODE_SEG, KERNEL_DATA_SEG, USER_CODE_SEG, USER_DATA_SEG,
    };
    let (cs, ss) = match mode {
        TaskMode::Kernel => (KERNEL_CODE_SEG as u64, KERNEL_DATA_SEG as u64),
        TaskMode::User => ((USER_CODE_SEG | 0x3) as u64, (USER_DATA_SEG | 0x3) as u64),
    };
    FullContext {
        regs: unsafe { core::mem::zeroed() },
        frame: IretFrame {
            rip: entry as u64,
            cs,
            // 0x202 = interrupts enabled (IF flag set)
            rflags: 0x202,
            rsp: stack_top,
            ss,
        },
    }
}
