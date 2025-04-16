#![no_std]
#![no_main]

// extern crate alloc;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn bang(kind_ptr: *const u8, kind_len: usize) -> u64;
    fn mutate(id: u64, key_ptr: *const u8, key_len: usize, val_ptr: *const u8, val_len: usize);
    fn poof(id: u64);
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    let kind = "message";
    let key = "text";
    let value = "Tada!";

    unsafe {
        let id = bang(kind.as_ptr(), kind.len());
        mutate(id, key.as_ptr(), key.len(), value.as_ptr(), value.len());
        // poof(id);
    }
}
