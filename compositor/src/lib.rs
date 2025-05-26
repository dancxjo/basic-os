#![no_std]
#[macro_use]
mod prelude;
use crate::prelude::*;

const WIDTH: usize = 100;
const HEIGHT: usize = 100;
static mut RECT: [u32; WIDTH * HEIGHT] = [0xFF0000FF; WIDTH * HEIGHT]; // ARGB: Red

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    println!("Yo no soy marinero, soy capitán");
    loop {
        unsafe {
            #![allow(static_mut_refs)]
            blit(200, 150, WIDTH, HEIGHT, RECT.as_ptr());
        }
        for _ in 0..100_000_000 {} // Delay
    }
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("\nPanic!");
    loop {}
}
