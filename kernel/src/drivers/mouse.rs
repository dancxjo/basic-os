use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::drivers::device::{self, DeviceKind, MOUSE_DEVICE_NAME};
use crate::drivers::input::InputBuffer;
use crate::graph::canon;
use alloc::collections::BTreeMap;
use log::warn;
use thing_abi::Value;
use uuid::Uuid;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub const MOUSE_RAW_CAPACITY: usize = 4096;

pub static MOUSE_RAW_BYTES: InputBuffer<u8, MOUSE_RAW_CAPACITY> = InputBuffer::new(0);

fn mouse_device_id() -> Uuid {
    device::device_uuid(MOUSE_DEVICE_NAME)
}

pub fn read_mouse(buf: &mut [u8]) -> usize {
    let mut written = 0;
    for slot in buf.iter_mut() {
        match MOUSE_RAW_BYTES.pop() {
            Some(byte) => {
                *slot = byte;
                written += 1;
            }
            None => break,
        }
    }
    written
}

/// Register the PS/2 mouse as a device endpoint.
pub fn init() -> Result<(), &'static str> {
    enable_irq();
    let mut fields = BTreeMap::new();
    fields.insert(canon::IRQ_LINE, Value::U64(12));
    fields.insert(canon::KIND, Value::Text("mouse".into()));
    fields.insert(canon::MODE, Value::Text("native".into()));
    let node = device::create_device_node(canon::DEVICE, MOUSE_DEVICE_NAME, fields);
    device::register_device(DeviceKind::Mouse, Some(read_mouse), None, None, Some(node));
    Ok(())
}

/// Low-level IRQ handler. Buffers raw bytes for legacy readers and emits graph-native events
/// for the bound mouse driver bundles.
pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut data_port = Port::<u8>::new(0x60);
    let packet: u8 = unsafe { data_port.read() };

    if MOUSE_RAW_BYTES.push(packet).is_err() {
        warn!("Mouse packet buffer overflow");
    }

    end_of_interrupt(12);
}

fn enable_irq() {
    unsafe {
        wait_input_ready();
        Port::<u8>::new(0x64).write(0xA8); // Enable aux device

        wait_input_ready();
        Port::<u8>::new(0x64).write(0x20); // Read command byte

        wait_output_ready();
        let status = Port::<u8>::new(0x60).read();

        wait_input_ready();
        Port::<u8>::new(0x64).write(0x60); // Write command byte

        wait_input_ready();
        Port::<u8>::new(0x60).write(status | 2); // Enable IRQ12

        wait_input_ready();
        Port::<u8>::new(0x64).write(0xD4); // Write to mouse

        wait_input_ready();
        Port::<u8>::new(0x60).write(0xF4); // Enable data reporting

        wait_output_ready();
        let _ack = Port::<u8>::new(0x60).read(); // Should be 0xFA
    }
}

fn wait_input_ready() {
    while unsafe { Port::<u8>::new(0x64).read() } & 0x02 != 0 {}
}

fn wait_output_ready() {
    while unsafe { Port::<u8>::new(0x64).read() } & 0x01 == 0 {}
}
