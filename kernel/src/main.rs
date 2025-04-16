#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

extern crate alloc;

mod bootloader;
mod fiat;
mod gdt;
mod idt;
mod kernel;
mod memory;
mod message;
mod overlay;
mod panic;
mod seed;
mod serial;
mod thing;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    let mut k = kernel::default(); // calls ::new({ new empty vecs for indexes })
    k.spin();
}
