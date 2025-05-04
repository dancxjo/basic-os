const TRAMPOLINE_ADDR: usize = 0x7000;
static TRAMPOLINE: &[u8] = include_bytes!("../ap_trampoline.bin");

pub fn load_trampoline() {
    unsafe {
        core::ptr::copy_nonoverlapping(
            TRAMPOLINE.as_ptr(),
            TRAMPOLINE_ADDR as *mut u8,
            TRAMPOLINE.len(),
        );
    }
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".ap_entry")]
pub extern "C" fn ap_main() {
    log::info!("Hello from AP CPU!");
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }
}

pub fn get_apic_id() -> u8 {
    let cpuid = raw_cpuid::CpuId::new();
    cpuid
        .get_feature_info()
        .map(|f| ((f.initial_local_apic_id()) & 0xFF) as u8)
        .unwrap_or(0)
}
