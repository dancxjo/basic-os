use heapless::String;
use log::{Level, Metadata, Record};
use spin::Mutex;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: Level,
    pub message: String<128>,
}
