use core::fmt;

/// Raw syscall entry point (rax, rdi, rsi, rdx).
#[inline(always)]
pub unsafe fn syscall(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") rax => ret,
        inout("rdi") rdi => _,
        inout("rsi") rsi => _,
        inout("rdx") rdx => _,
        out("rcx") _,
        out("r11") _,
        out("r8") _,
        out("r9") _,
        out("r10") _,
        out("xmm0") _,
        out("xmm1") _,
        out("xmm2") _,
        out("xmm3") _,
        out("xmm4") _,
        out("xmm5") _,
        out("xmm6") _,
        out("xmm7") _,
        out("xmm8") _,
        out("xmm9") _,
        out("xmm10") _,
        out("xmm11") _,
        out("xmm12") _,
        out("xmm13") _,
        out("xmm14") _,
        out("xmm15") _,
        options(nostack)
    );
    ret
}

pub const SYSCALL_GRAPH_FIAT: u64 = 0x01;
pub const SYSCALL_GRAPH_LINK: u64 = 0x02;
pub const SYSCALL_GRAPH_QUERY: u64 = 0x03;
// pub const SYSCALL_GRAPH_WATCH: u64 = 0x04;
pub const SYSCALL_GRAPH_GET: u64 = 0x05;
pub const SYSCALL_WATCH_REGISTER: u64 = 0x06;
pub const SYSCALL_WATCH_POLL: u64 = 0x07;
pub const SYSCALL_KBD_READ: u64 = 0x08;

pub fn graph_fiat_raw(payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_FIAT,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
        )
    }
}

pub fn graph_snapshot_raw(out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_QUERY,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
        )
    }
}

pub fn graph_link_raw(payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_LINK,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
        )
    }
}

pub fn watch_register_raw(payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_WATCH_REGISTER,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
        )
    }
}

pub fn watch_poll_raw(watch_id: u64, out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_WATCH_POLL,
            watch_id,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        )
    }
}

pub fn kbd_read_raw(out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_KBD_READ,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
        )
    }
}

pub fn graph_get_raw(id_bytes: &[u8; 16], out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_GET,
            id_bytes.as_ptr() as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        )
    }
}

pub fn print_fmt(args: fmt::Arguments) {
    crate::graph::log_args(args);
}
