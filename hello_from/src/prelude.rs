use core::fmt::{self, Write};

unsafe extern "C" {
    fn putchar(c: u8);
    fn get_pid() -> u32;
    fn get_char() -> u8;
}

struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            unsafe {
                putchar(b);
            }
        }
        Ok(())
    }
}

pub fn print_fmt(args: fmt::Arguments) {
    let _ = Console.write_fmt(args);
}

pub fn pid() -> u32 {
    unsafe { get_pid() }
}

pub fn getchar() -> u8 {
    unsafe { get_char() }
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
