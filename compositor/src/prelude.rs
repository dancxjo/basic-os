use core::fmt::{self, Write};

unsafe extern "C" {
    pub fn blit(x: usize, y: usize, w: usize, h: usize, data: *const u32) -> i32;
    pub fn putchar(ch: u8);
}

pub struct Console;

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
