#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    userland::ensure_kernel_runtime();
    init::app_main()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    userland::println!("Init Panic: {}", info);
    loop {}
}
