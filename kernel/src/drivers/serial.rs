//! Serial I/O with synchronized access

use crate::drivers::device::{self, DeviceKind};
use core::fmt::{self, Write};
use spin::Mutex;
use x86_64::instructions::port::Port;

pub struct SerialPort {
    port: Port<u8>,
}

impl SerialPort {
    pub const fn new(port: u16) -> Self {
        Self {
            port: Port::new(port),
        }
    }

    pub fn init(&mut self) {
        unsafe {
            self.port.write(0x00); // Disable all interrupts
            self.port.write(0x80); // Enable DLAB
            self.port.write(0x03); // Set divisor to 3 (38400 baud)
            self.port.write(0x00);
            self.port.write(0x03); // 8 bits, no parity, one stop bit
            self.port.write(0xC7); // Enable FIFO, clear them, with 14-byte threshold
            self.port.write(0x0B); // IRQs enabled, RTS/DSR set
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        unsafe {
            self.port.write(byte);
        }
    }
}

impl Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = Mutex::new(SerialPort::new(0x3F8));
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\r\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\r\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(concat!($fmt, "\r\n"), $($arg)*));
}

#[macro_export]
macro_rules! klog {
    ($($arg:tt)*) => {
        $crate::serial_println!($($arg)*);
    };
}

#[macro_export]
macro_rules! klog_raw {
    ($msg:expr) => {
        unsafe { $crate::drivers::serial::raw_write($msg.as_bytes()) };
    };
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    if let Some(mut serial) = SERIAL1.try_lock() {
        let _ = serial.write_fmt(args);
    }
}

/// Unsafe raw write to serial port, bypassing locks.
/// Useful for panic handlers and low-level debugging.
pub unsafe fn raw_write(buf: &[u8]) {
    let mut port = Port::<u8>::new(0x3F8);
    let mut status = Port::<u8>::new(0x3F8 + 5);
    for &b in buf {
        // Wait for THRE (Transmitter Holding Register Empty)
        unsafe {
            while status.read() & 0x20 == 0 {}
            port.write(b);
        }
    }
}

/// Writes a single byte to COM1 in interrupt context.
/// Must not take any lock or allocate.
/// May busy-wait briefly on the UART transmit empty bit.
#[unsafe(no_mangle)]
pub extern "C" fn serial_debug_putc_irq(ch: u8) {
    unsafe {
        let mut port = Port::<u8>::new(0x3F8);
        let mut status = Port::<u8>::new(0x3F8 + 5);
        // Wait for THRE (Transmitter Holding Register Empty)
        while status.read() & 0x20 == 0 {}
        port.write(ch);
    }
}

#[macro_export]
macro_rules! klog_irq {
    ($ch:expr) => {{
        $crate::drivers::serial::serial_debug_putc_irq($ch as u8);
    }};
}

pub unsafe fn raw_write_hex(mut val: u64) {
    let mut buf = [0u8; 18]; // "0x" + 16 digits
    buf[0] = b'0';
    buf[1] = b'x';
    let hex = b"0123456789ABCDEF";
    for i in (0..16).rev() {
        buf[2 + i] = hex[(val & 0xF) as usize];
        val >>= 4;
    }
    unsafe { raw_write(&buf) };
}

fn write_serial(buf: &[u8]) -> usize {
    let mut guard = SERIAL1.lock();
    for byte in buf {
        guard.write_byte(*byte);
    }
    buf.len()
}

pub fn init_serial() {
    SERIAL1.lock().init();
    device::register_device(DeviceKind::Serial, None, Some(write_serial), None, None);
}
