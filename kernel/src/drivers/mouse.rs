use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::drivers::device::{self, DeviceKind, MOUSE_DEVICE_NAME};
use crate::drivers::input::InputBuffer;
use crate::drivers::irq_dma;
use crate::graph::{self, GraphFiatRequest, canon, journal::Value};
use alloc::collections::BTreeMap;
use log::warn;
use spin::Mutex as SpinMutex;
use uuid::Uuid;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub const MOUSE_RAW_CAPACITY: usize = 4096;

pub static MOUSE_RAW_BYTES: InputBuffer<u8, MOUSE_RAW_CAPACITY> = InputBuffer::new(0);
static MOUSE_DECODER: SpinMutex<PacketDecoder> = SpinMutex::new(PacketDecoder::new());

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
    let node = device::create_device_node(canon::MOUSE_DEVICE, MOUSE_DEVICE_NAME, fields);
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

pub fn process_events() {
    loop {
        let packet = MOUSE_RAW_BYTES.pop();
        match packet {
            Some(p) => {
                let decoded = {
                    let mut decoder = MOUSE_DECODER.lock();
                    decoder.feed(p)
                };
                if let Some(event) = decoded {
                    irq_dma::for_each_binding(12, |binding| {
                        emit_mouse_event(event, binding);
                    });
                }
            }
            None => break,
        }
    }
}

fn emit_mouse_event(event: MouseEvent, binding: &irq_dma::IrqBindingInfo) {
    let device = mouse_device_id();
    if binding.device != device {
        return;
    }

    let ts = irq_dma::monotonic_ticks();

    let mut move_fields = BTreeMap::new();
    move_fields.insert(canon::DEVICE_ID, Value::Uuid(binding.device));
    move_fields.insert(canon::DX, Value::I64(event.dx as i64));
    move_fields.insert(canon::DY, Value::I64(event.dy as i64));
    move_fields.insert(canon::BUTTONS, Value::U64(event.buttons as u64));
    move_fields.insert(canon::TS, Value::U64(ts));

    let move_req = GraphFiatRequest {
        id: None,
        kind: canon::MOUSE_MOVE,
        fields: move_fields,
    };

    let _ = graph::fiat_for_bundle(binding.bundle, move_req);

    if event.buttons_changed {
        let mut button_fields = BTreeMap::new();
        button_fields.insert(canon::DEVICE_ID, Value::Uuid(binding.device));
        button_fields.insert(canon::BUTTONS, Value::U64(event.buttons as u64));
        button_fields.insert(canon::DOWN, Value::Bool(event.buttons != 0));
        button_fields.insert(canon::TS, Value::U64(ts));

        let button_req = GraphFiatRequest {
            id: None,
            kind: canon::MOUSE_BUTTON,
            fields: button_fields,
        };

        let _ = graph::fiat_for_bundle(binding.bundle, button_req);
    }
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
    buttons: u8,
    buttons_changed: bool,
}

#[derive(Clone, Copy)]
struct PacketDecoder {
    packet: [u8; 3],
    index: usize,
    buttons: u8,
}

impl PacketDecoder {
    const fn new() -> Self {
        Self {
            packet: [0; 3],
            index: 0,
            buttons: 0,
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

        let buttons = flags & 0x07;
        let buttons_changed = buttons != self.buttons;
        self.buttons = buttons;

        Some(MouseEvent {
            dx,
            dy,
            buttons,
            buttons_changed,
        })
    }
}
