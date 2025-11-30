#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    userland::ensure_kernel_runtime();
    rootfs::app_main()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    userland::println!("RootFS Panic: {}", info);
    loop {}
}
