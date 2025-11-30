#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

#[cfg(feature = "std")]
extern crate std as alloc;
#[cfg(not(feature = "std"))]
extern crate alloc;

use self_editing_demo::app_main;

#[cfg(not(feature = "std"))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    userland::init_heap();
    userland::ensure_kernel_runtime();
    app_main();
}

#[cfg(feature = "std")]
fn main() {
    app_main();
}

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use alloc::format;
    thingos_bundle_std::sys::log(&format!("Panic: {}\n", info));
    loop {}
}
