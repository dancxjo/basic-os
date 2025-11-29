use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::drivers::device::{self, DeviceKind, KEYBOARD_DEVICE_NAME};
use crate::drivers::input::InputBuffer;
use crate::graph::{self, canon, journal::Value};
use alloc::collections::BTreeMap;
use log::warn;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub const KEYBOARD_BUFFER_LEN: usize = 256;

pub static KEYBOARD_BUFFER: InputBuffer<u8, KEYBOARD_BUFFER_LEN> = InputBuffer::new(0);

fn keyboard_device_id() -> uuid::Uuid {
    device::device_uuid(KEYBOARD_DEVICE_NAME)
}

pub fn read_keyboard(buf: &mut [u8]) -> usize {
    let mut written = 0;
    for slot in buf.iter_mut() {
        match KEYBOARD_BUFFER.pop() {
            Some(byte) => {
                *slot = byte;
                written += 1;
            }
            None => break,
        }
    }
    written
}

/// Register the PS/2 keyboard as a device endpoint.
pub fn init() {
    let mut fields = BTreeMap::new();
    fields.insert(canon::IRQ_LINE, Value::U64(1));
    let node = device::create_device_node(canon::KEYBOARD_DEVICE, KEYBOARD_DEVICE_NAME, fields);
    device::register_device(
        DeviceKind::Keyboard,
        Some(read_keyboard),
        None,
        None,
        Some(node),
    );
}

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut data_port = Port::<u8>::new(0x60);
    let scancode: u8 = unsafe { data_port.read() };

    if KEYBOARD_BUFFER.push(scancode).is_err() {
        warn!(
            "Keyboard buffer overflow, dropping scancode 0x{:02X}",
            scancode
        );
    }

    end_of_interrupt(1);
}
