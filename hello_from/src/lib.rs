#![no_std]

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    loop {
        unsafe {
            putchar(b'H');
            putchar(b'e');
            putchar(b'l');
            putchar(b'l');
            putchar(b'o');
            putchar(b' ');
            putchar(b'f');
            putchar(b'r');
            putchar(b'o');
            putchar(b'm');
            putchar(b' ');
            putchar(b'T');
            putchar(b'a');
            putchar(b's');
            putchar(b'k');
            putchar(b' ');
            putchar(b'#');
            let pid = get_pid();
            putchar(b'0' + pid as u8); // naive ASCII conversion
        }
        for _ in 0..1_000_000_000 {
            // Busy wait to simulate work
        }
        unsafe {
            putchar(b'\n');
        }
    }
}

unsafe extern "C" {
    fn putchar(c: u8);
    fn get_pid() -> u32;
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    let msg = b"\nPanic!\n";
    for &c in msg {
        unsafe {
            putchar(c);
        }
    }
    loop {}
}
