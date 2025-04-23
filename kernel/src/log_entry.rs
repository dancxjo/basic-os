use heapless::String;
use log::Level;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: Level,
    pub message: String<128>,
}
