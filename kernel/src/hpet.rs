pub struct HPET {
    base: u64,
}

impl HPET {
    pub fn new() -> Self {
        let base = get_hpet_base().unwrap();
        Self { base }
    }

    pub fn read(&self) -> u64 {
        unsafe {
            let ptr = (self.base + 0xF0) as *const u64; // Main counter
            core::ptr::read_volatile(ptr)
        }
    }
}

fn get_hpet_base() -> Option<u64> {
    // TODO: Parse ACPI to find the HPET base address
    Some(0xFED00000u64)
}
