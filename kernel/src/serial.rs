#[allow(static_mut_refs)]
use core::fmt::{self, Write};
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

pub static mut SERIAL1: SerialPort = SerialPort::new(0x3F8);

#[macro_export]
macro_rules! serial_println {
    ($($arg:tt)*) => {
            use core::fmt::Write;
            #[allow(static_mut_refs)]
            let _ = writeln!(crate::serial::SERIAL1, $($arg)*);
    };
}
