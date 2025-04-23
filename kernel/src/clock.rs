use core::fmt::{self, Display, Formatter};

use alloc::boxed::Box;
use log::info;
use serde::Serialize;
use thing_macros::Kind;
use x86_64::instructions::port::Port;

#[derive(Debug, Clone, Serialize)]
pub struct Clock {
    hpet: HPET,
    rtc: RTC,
    start_ticks: u64,
    period_fs: u64,
    boot_time: Moment,
}

impl Clock {
    pub fn new(hpet: HPET, rtc: RTC) -> Self {
        let caps = hpet.capabilities();
        info!("HPET Capabilities: {:#x}", caps);

        let period_fs = (caps >> 32) & 0xFFFF_FFFF;
        hpet.enable();

        let boot_time = rtc.read_time();
        let start_ticks = hpet.read();

        info!("HPET Start: {}", start_ticks);
        info!("RTC Boot Time: {}", boot_time.to_string());

        Self {
            hpet,
            rtc,
            start_ticks,
            period_fs,
            boot_time,
        }
    }

    pub fn ticks_since_boot(&self) -> u64 {
        self.hpet.read().saturating_sub(self.start_ticks)
    }

    pub fn nanos_since_boot(&self) -> u64 {
        let ticks = self.ticks_since_boot();
        (ticks * self.period_fs) / 1_000_000
    }

    pub fn current_time(&self) -> Moment {
        // crude estimation
        let mut now = self.boot_time;
        let elapsed = self.nanos_since_boot() / 1_000_000_000;

        now.second = now.second.saturating_add(elapsed as u8);
        // overflow-safe and BCD-happy logic omitted for brevity
        now
    }

    pub fn booted_at(&self) -> Moment {
        self.boot_time
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HPET {
    base: u64,
}

impl HPET {
    pub fn new(base: u64) -> Self {
        Self { base }
    }

    pub fn capabilities(&self) -> u64 {
        unsafe { core::ptr::read_volatile((self.base + 0x00) as *const u64) }
    }

    pub fn enable(&self) {
        unsafe {
            let ptr = (self.base + 0x10) as *mut u64;
            let mut val = core::ptr::read_volatile(ptr);
            val |= 1;
            core::ptr::write_volatile(ptr, val);
        }
    }

    pub fn read(&self) -> u64 {
        unsafe { core::ptr::read_volatile((self.base + 0xF0) as *const u64) }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RTC;

impl RTC {
    pub fn new() -> Self {
        RTC
    }

    fn read_register(reg: u8) -> u8 {
        unsafe {
            let mut addr = Port::new(0x70);
            let mut data = Port::new(0x71);
            addr.write(reg);
            data.read()
        }
    }

    pub fn read_time(&self) -> Moment {
        while Self::read_register(0x0A) & 0x80 != 0 {}
        let bcd = Self::read_register(0x0B) & 0x04 == 0;

        let second = Self::read_register(0x00);
        let minute = Self::read_register(0x02);
        let hour = Self::read_register(0x04);
        let day = Self::read_register(0x07);
        let month = Self::read_register(0x08);
        let year = Self::read_register(0x09);

        Moment {
            second: if bcd { bcd_to_bin(second) } else { second },
            minute: if bcd { bcd_to_bin(minute) } else { minute },
            hour: if bcd { bcd_to_bin(hour) } else { hour },
            day: if bcd { bcd_to_bin(day) } else { day },
            month: if bcd { bcd_to_bin(month) } else { month },
            year: if bcd {
                2000 + bcd_to_bin(year) as u16
            } else {
                2000 + year as u16
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Moment {
    pub second: u8,
    pub minute: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

impl Moment {
    pub fn new(second: u8, minute: u8, hour: u8, day: u8, month: u8, year: u16) -> Self {
        Moment {
            second,
            minute,
            hour,
            day,
            month,
            year,
        }
    }

    pub fn to_string(&self) -> heapless::String<32> {
        use core::fmt::Write;
        let mut s = heapless::String::new();
        write!(
            &mut s,
            "{:02}/{:02}/{} {:02}:{:02}:{:02}",
            self.month, self.day, self.year, self.hour, self.minute, self.second
        )
        .unwrap();
        s
    }
}

impl Display for Moment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}/{:02}/{} {:02}:{:02}:{:02}",
            self.month, self.day, self.year, self.hour, self.minute, self.second
        )
    }
}

fn bcd_to_bin(val: u8) -> u8 {
    ((val / 16) * 10) + (val & 0x0F)
}
