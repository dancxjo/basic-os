use core::arch::asm;
use core::panic::PanicInfo;

use crate::serial_println;

#[panic_handler]
fn rust_panic(info: &PanicInfo) -> ! {
    #[allow(static_mut_refs)]
    unsafe {
        crate::serial::SERIAL1.init();
        serial_println!("Kernel panic: {info}");
    }
    hcf();
}

fn hcf() -> ! {
    loop {
        unsafe {
            #[cfg(target_arch = "x86_64")]
            asm!("hlt");
            #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
            asm!("wfi");
            #[cfg(target_arch = "loongarch64")]
            asm!("idle 0");
        }
    }
}
