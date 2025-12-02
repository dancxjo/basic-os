use heapless::{String, spsc::Queue};
use log::{Metadata, Record};
use spin::Mutex;

use crate::{logging::log_entry::LogEntry, serial_println};

/// Full logging macros (info!, kinfo!, etc.) are not allowed in interrupt context or scheduler critical paths.
/// Use klog_irq! for IRQ-safe trace breadcrumbs.
pub struct KernelLogger {
    buffer: Mutex<Queue<LogEntry, 64>>,
}

impl KernelLogger {
    pub const fn new() -> Self {
        Self {
            buffer: Mutex::new(Queue::new()),
        }
    }

    pub fn pop(&self) -> Option<LogEntry> {
        self.buffer.lock().dequeue()
    }

    pub fn iter(&self) -> heapless::Vec<LogEntry, 64> {
        let buf = self.buffer.lock();
        buf.iter().cloned().collect()
    }
}

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        // Serial log (fast path; avoid heapless formatting failure panicking)
        serial_println!("[{}] {}", record.level(), record.args());

        // Avoid potential deadlocks when logging from IRQ context by only
        // attempting a non-blocking lock on the buffer.
        if let Some(mut buf) = self.buffer.try_lock() {
            use core::fmt::Write;
            let mut msg = String::<128>::new();
            let _ = write!(msg, "{}", record.args());

            let entry = LogEntry {
                level: record.level(),
                message: msg,
            };

            let _ = buf.enqueue(entry); // silently drop oldest if full
        }
    }

    fn flush(&self) {}
}

static LOGGER: KernelLogger = KernelLogger::new();

pub fn init_logger() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .expect("logger setup failed");
}

pub fn logger() -> &'static KernelLogger {
    &LOGGER
}
