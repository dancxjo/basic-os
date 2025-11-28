//! Task context structures for x86_64.
//!
//! These structures define the saved CPU state for context switching.
//! The layout must match the assembly code in tick_handler.S and restore_context.S.

/// Task execution mode (privilege level).
#[derive(Debug, Clone, Copy, PartialEq)]
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
/// rax, rbx, rcx, rdx, rbp, rdi, rsi, r8, r9, r10, r11, r12, r13, r14, r15,
/// the stack layout (from low address to high) becomes:
/// r15, r14, r13, r12, r11, r10, r9, r8, rsi, rdi, rbp, rdx, rcx, rbx, rax.
///
/// Offsets from start of struct:
/// - r15: offset 0
/// - r14: offset 8
/// - ...
/// - rax: offset 112 (14 * 8)
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
    use crate::arch::x86_64::gdt::SELECTORS;
    #[allow(static_mut_refs)]
    let selectors = unsafe { SELECTORS.as_ref().expect("GDT not initialized") };
    let kernel_cs = (selectors.code.0 & !0x3) as u64;
    let kernel_ss = (selectors.data.0 & !0x3) as u64;
    let user_cs = (selectors.user_code.0 | 0x3) as u64;
    let user_ss = (selectors.user_data.0 | 0x3) as u64;
    let (cs, ss, rflags) = match mode {
        TaskMode::Kernel => (kernel_cs, kernel_ss, 0x2),
        TaskMode::User => (user_cs, user_ss, 0x202),
    };
    FullContext {
        regs: unsafe { core::mem::zeroed() },
        frame: IretFrame {
            rip: entry as u64,
            cs,
            rflags,
            rsp: stack_top,
            ss,
        },
    }
}
