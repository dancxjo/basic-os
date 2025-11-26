use core::fmt::{self, Write};

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

const fn canon(a: u8, b: u8, c: u8) -> u32 {
    ((a as u32) << 16) | ((b as u32) << 8) | (c as u32)
}

pub const SYSCALL_WRITE_PORT: u64 = 0x01;
pub const _SYSCALL_READ_PORT: u64 = 0x02;
pub const SYSCALL_JOURNAL_EMIT: u64 = 0x10;
pub const SYSCALL_JOURNAL_SNAPSHOT: u64 = 0x11;
pub const SYSCALL_GRAPH_SNAPSHOT: u64 = 0x12;
pub const SYSCALL_DEV_OPEN: u64 = 0x20;
pub const SYSCALL_DEV_READ: u64 = 0x21;
pub const SYSCALL_DEV_WRITE: u64 = 0x22;
pub const SYSCALL_DEV_MAP: u64 = 0x23;

const PORT_CONSOLE_OUT: u64 = 1;
const _PORT_CONSOLE_IN: u64 = 2;
const KIND_WRITE: u64 = canon(b'W', b'R', b'T') as u64;

pub fn putchar(c: u8) {
    unsafe {
        syscall(SYSCALL_WRITE_PORT, PORT_CONSOLE_OUT, c as u64, 0);
    }
}

pub fn journal_emit_raw(kind: u32, payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_JOURNAL_EMIT,
            kind as u64,
            payload.as_ptr() as u64,
            payload.len() as u64,
        )
    }
}

pub fn journal_snapshot_raw(out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_JOURNAL_SNAPSHOT,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
        )
    }
}

pub fn graph_snapshot_raw(out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_SNAPSHOT,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceMapping {
    pub addr: u64,
    pub len: usize,
}

pub fn dev_open(kind: u32, index: u32) -> Option<u64> {
    let ret = unsafe { syscall(SYSCALL_DEV_OPEN, kind as u64, index as u64, 0) };
    if ret == !0 {
        None
    } else {
        Some(ret)
    }
}

pub fn dev_read(handle: u64, out: &mut [u8]) -> usize {
    if out.is_empty() {
        return 0;
    }
    unsafe {
        syscall(
            SYSCALL_DEV_READ,
            handle,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        ) as usize
    }
}

pub fn dev_write(handle: u64, buf: &[u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }
    unsafe {
        syscall(
            SYSCALL_DEV_WRITE,
            handle,
            buf.as_ptr() as u64,
            buf.len() as u64,
        ) as usize
    }
}

pub fn dev_map(handle: u64) -> Option<DeviceMapping> {
    let mut len_out: u64 = 0;
    let addr = unsafe { syscall(SYSCALL_DEV_MAP, handle, &mut len_out as *mut u64 as u64, 0) };
    if addr == 0 || len_out == 0 {
        None
    } else {
        Some(DeviceMapping {
            addr,
            len: len_out as usize,
        })
    }
}

pub struct Console;

fn emit_write_event(s: &str) -> bool {
    let res = unsafe {
        syscall(
            SYSCALL_JOURNAL_EMIT,
            KIND_WRITE,
            s.as_ptr() as u64,
            s.len() as u64,
        )
    };
    res == 0
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if emit_write_event(s) {
            return Ok(());
        }

        for b in s.bytes() {
            putchar(b);
        }
        Ok(())
    }
}

pub fn print_fmt(args: fmt::Arguments) {
    let _ = Console.write_fmt(args);
}
