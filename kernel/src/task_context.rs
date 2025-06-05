pub enum TaskMode {
    Kernel,
    User,
}

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
#[derive(Debug, Clone, Copy)]
pub struct FullContext {
    pub regs: GeneralRegisters,
    pub frame: IretFrame,
}

pub fn prepare_context(entry: extern "C" fn(), stack_top: u64, mode: TaskMode) -> FullContext {
    use crate::gdt::{KERNEL_CODE_SEG, KERNEL_DATA_SEG, USER_CODE_SEG, USER_DATA_SEG};
    let (cs, ss) = match mode {
        TaskMode::Kernel => (KERNEL_CODE_SEG as u64, KERNEL_DATA_SEG as u64),
        TaskMode::User => ((USER_CODE_SEG | 0x3) as u64, (USER_DATA_SEG | 0x3) as u64),
    };
    FullContext {
        regs: unsafe { core::mem::zeroed() },
        frame: IretFrame {
            rip: entry as u64,
            cs,
            rflags: 0x202,
            rsp: stack_top,
            ss,
        },
    }
}
