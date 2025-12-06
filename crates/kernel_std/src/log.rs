#[cfg(feature = "std")]
use thing_model::env::Log;

pub fn info(msg: &str) {
    log::info!("{}", msg);
}

pub fn warn(msg: &str) {
    log::warn!("{}", msg);
}

pub fn error(msg: &str) {
    log::error!("{}", msg);
}

#[macro_export]
macro_rules! klog {
    ($level:expr, $($arg:tt)*) => {
        match $level {
            "INFO" => log::info!($($arg)*),
            "WARN" => log::warn!($($arg)*),
            "ERROR" => log::error!($($arg)*),
            _ => log::info!($($arg)*),
        }
    };
}

#[macro_export]
macro_rules! kinfo {
    ($($arg:tt)*) => {
        log::info!($($arg)*);
    };
}

#[macro_export]
macro_rules! kwarn {
    ($($arg:tt)*) => {
        log::warn!($($arg)*);
    };
}

#[macro_export]
macro_rules! kerror {
    ($($arg:tt)*) => {
        log::error!($($arg)*);
    };
}

#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, Default)]
pub struct StdLog;

#[cfg(feature = "std")]
impl Log for StdLog {
    fn info(&self, msg: &str) {
        info(msg);
    }

    fn warn(&self, msg: &str) {
        warn(msg);
    }

    fn error(&self, msg: &str) {
        error(msg);
    }
}
