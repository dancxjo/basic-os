#![no_std]
#![no_main]

use core::panic::PanicInfo;

const FRAMEBUFFER_ADDR: usize = 0xFFFF_FF00_0000_0000;
const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const PITCH: usize = WIDTH * 4;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let fb = FRAMEBUFFER_ADDR as *mut u32;

    // Draw a 50x50 red square
    for y in 0..50 {
        for x in 0..50 {
            unsafe {
                let offset = (y * WIDTH + x) as isize;
                fb.offset(offset).write_volatile(0x00FF0000); // Red in 0xAABBGGRR
            }
        }
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
