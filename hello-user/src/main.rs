#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    unsafe {
        *(0x500000 as *mut u64) = 0xB16B00B5B16B00B5;
    }
}
