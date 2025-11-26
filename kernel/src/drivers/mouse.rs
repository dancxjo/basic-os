use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::drivers::device::{self, DeviceKind};
use crate::drivers::input::InputBuffer;
use crate::telemetry::{
    canon,
    graph::{self, GraphFiatRequest},
    journal::Value,
};
use alloc::collections::BTreeMap;
use log::warn;
use spin::Mutex as SpinMutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub const MOUSE_RAW_CAPACITY: usize = 4096;

pub static MOUSE_RAW_BYTES: InputBuffer<u8, MOUSE_RAW_CAPACITY> = InputBuffer::new(0);
static MOUSE_DECODER: SpinMutex<PacketDecoder> = SpinMutex::new(PacketDecoder::new());

fn read_mouse(buf: &mut [u8]) -> usize {
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
    device::register_device(DeviceKind::Mouse, Some(read_mouse), None, None);
    Ok(())
}

/// Low-level IRQ handler. Only buffers raw bytes; higher-level decoding lives in userland.
pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut data_port = Port::<u8>::new(0x60);
    let packet: u8 = unsafe { data_port.read() };

    if MOUSE_RAW_BYTES.push(packet).is_err() {
        warn!("Mouse packet buffer overflow");
    }

    if let Some(event) = MOUSE_DECODER.lock().feed(packet) {
        publish_mouse_event(event);
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

#[derive(Clone, Copy, Debug, Default)]
struct MouseEvent {
    dx: i8,
    dy: i8,
    left: bool,
    right: bool,
    middle: bool,
}

#[derive(Clone, Copy)]
struct PacketDecoder {
    packet: [u8; 3],
    index: usize,
}

impl PacketDecoder {
    const fn new() -> Self {
        Self {
            packet: [0; 3],
            index: 0,
        }
    }

    fn feed(&mut self, byte: u8) -> Option<MouseEvent> {
        if self.index == 0 && byte & 0x08 == 0 {
            return None;
        }

        self.packet[self.index] = byte;
        self.index = (self.index + 1) % 3;

        if self.index != 0 {
            return None;
        }

        let flags = self.packet[0];
        let dx = self.packet[1] as i8;
        let dy = (self.packet[2] as i8).wrapping_neg();

        if flags & 0x40 != 0 || flags & 0x80 != 0 {
            return None;
        }

        Some(MouseEvent {
            dx,
            dy,
            left: flags & 0x01 != 0,
            right: flags & 0x02 != 0,
            middle: flags & 0x04 != 0,
        })
    }
}

fn publish_mouse_event(event: MouseEvent) {
    let mut fields = BTreeMap::new();
    fields.insert(canon::DX, Value::I64(event.dx as i64));
    fields.insert(canon::DY, Value::I64(event.dy as i64));
    let buttons: u8 = (event.left as u8) | ((event.right as u8) << 1) | ((event.middle as u8) << 2);
    fields.insert(canon::BUTTONS, Value::U64(buttons as u64));

    let req = GraphFiatRequest {
        id: None,
        kind: canon::MOUSE_MOVED,
        fields,
    };
    let _ = graph::fiat(req);
}
