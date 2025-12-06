pub trait Log {
    fn info(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn error(&self, msg: &str);
}

pub trait Clock {
    fn now_monotonic(&self) -> u64;
}
