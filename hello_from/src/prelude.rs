use core::fmt::{self, Write};

/// Syscall interface (matching kernel syscall IDs)
#[inline(always)]
unsafe fn syscall(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") rax => ret,
        in("rdi") rdi,
        in("rsi") rsi,
        in("rdx") rdx,
        options(nostack)
    );
    ret
}

const SYSCALL_WRITE_PORT: u64 = 0x01;
const _SYSCALL_READ_PORT: u64 = 0x02;

const PORT_CONSOLE_OUT: u64 = 1;
const _PORT_CONSOLE_IN: u64 = 2;

pub fn putchar(c: u8) {
    unsafe {
        syscall(SYSCALL_WRITE_PORT, PORT_CONSOLE_OUT, c as u64, 0);
    }
}

pub fn _getchar() -> u8 {
    unsafe { syscall(_SYSCALL_READ_PORT, _PORT_CONSOLE_IN, 0, 0) as u8 }
}

pub struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            putchar(b);
        }
        Ok(())
    }
}

pub fn print_fmt(args: fmt::Arguments) {
    let _ = Console.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::prelude::print_fmt(core::format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {
        $crate::print!("{}\n", core::format_args!($($arg)*));
    };
}
