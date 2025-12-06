#[cfg(feature = "std")]
use std::time::Instant as HostInstant;
#[cfg(feature = "std")]
use thing_model::env::Clock;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Instant(pub u64);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Duration(pub u64);

// TODO: Integrate with real kernel time source

#[cfg(feature = "std")]
#[derive(Clone, Debug)]
pub struct StdClock {
    start: HostInstant,
}

#[cfg(feature = "std")]
impl StdClock {
    pub fn new() -> Self {
        Self {
            start: HostInstant::now(),
        }
    }
}

#[cfg(feature = "std")]
impl Default for StdClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl Clock for StdClock {
    fn now_monotonic(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }
}
