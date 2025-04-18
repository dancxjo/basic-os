#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

use kernel::Kernel;

extern crate alloc;

mod bootloader;
mod fiat;
mod framebuffer;
mod gdt;
mod idt;
mod interrupts;
mod kernel;
mod memory;
mod message;
mod mouse;
mod os_space;
mod overlay;
mod panic;
mod screen;
mod seed;
mod serial;
mod system_graph;
mod thing;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    let mut k = Kernel::new();
    k.spin();
}
