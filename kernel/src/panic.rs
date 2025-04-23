use core::arch::asm;
use core::panic::PanicInfo;

use log::error;

#[panic_handler]
fn rust_panic(info: &PanicInfo) -> ! {
    error!("Kernel panic: {}", info);
    halt();
}

pub fn halt() -> ! {
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
