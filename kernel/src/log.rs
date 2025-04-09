// use crate::serial_println;
use core::fmt::{self};

use alloc::string::ToString;
use spinning_top::Spinlock;

use crate::message_queue::MessageQueue;

pub static LOG_MESSAGES: Spinlock<MessageQueue> = Spinlock::new(MessageQueue::new());

pub fn log(args: core::fmt::Arguments) {
    let msg = args.to_string();
    LOG_MESSAGES.lock().push(msg);
    // unsafe {
    // crate::serial::SERIAL1.init();
    // serial_println!(msg);
    // }
}

pub fn warn(args: fmt::Arguments) {
    log(format_args!("[WARN] {}", args));
}

// Convenience macro
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::log::log(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
}
