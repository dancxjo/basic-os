use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::drivers::device::{self, DeviceKind, KEYBOARD_DEVICE_NAME};
use crate::drivers::input::InputBuffer;
use crate::drivers::irq_dma;
use crate::telemetry::graph::{self, GraphFiatRequest};
use crate::telemetry::{canon, journal::Value};
use alloc::collections::BTreeMap;
use log::warn;
use uuid::Uuid;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub const KEYBOARD_BUFFER_LEN: usize = 256;

pub static KEYBOARD_BUFFER: InputBuffer<u8, KEYBOARD_BUFFER_LEN> = InputBuffer::new(0);

fn keyboard_device_id() -> Uuid {
    device::device_uuid(KEYBOARD_DEVICE_NAME)
}

fn read_keyboard(buf: &mut [u8]) -> usize {
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

    let bindings = irq_dma::notify_irq(1);
    emit_key_events(scancode, &bindings);

    end_of_interrupt(1);
}

pub fn read_scancodes(buf: &mut [u8]) -> usize {
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

fn emit_key_events(scancode: u8, bindings: &[irq_dma::IrqBindingInfo]) {
    if bindings.is_empty() {
        return;
    }

    let (code, down) = decode_scancode(scancode);
    let key_text = scancode_text(code);
    let ts = irq_dma::monotonic_ticks();
    let device = keyboard_device_id();

    for binding in bindings.iter().filter(|b| b.device == device) {
        let mut fields = BTreeMap::new();
        fields.insert(canon::DEVICE_ID, Value::Uuid(binding.device));
        fields.insert(canon::SCANCODE, Value::U64(code as u64));
        fields.insert(canon::DOWN, Value::Bool(down));
        fields.insert(canon::TS, Value::U64(ts));
        if let Some(text) = key_text {
            fields.insert(canon::TEXT, Value::Text(text.into()));
        }

        let req = GraphFiatRequest {
            id: None,
            kind: canon::KEY_EVENT,
            fields,
        };

        let _ = graph::fiat_for_bundle(binding.bundle, req);
    }
}

fn decode_scancode(scancode: u8) -> (u8, bool) {
    if scancode & 0x80 != 0 {
        (scancode & 0x7F, false)
    } else {
        (scancode, true)
    }
}

fn scancode_text(scancode: u8) -> Option<&'static str> {
    match scancode {
        0x1E => Some("a"),
        0x30 => Some("b"),
        0x2E => Some("c"),
        0x20 => Some("d"),
        0x12 => Some("e"),
        0x21 => Some("f"),
        0x22 => Some("g"),
        0x23 => Some("h"),
        0x17 => Some("i"),
        0x24 => Some("j"),
        0x25 => Some("k"),
        0x26 => Some("l"),
        0x32 => Some("m"),
        0x31 => Some("n"),
        0x18 => Some("o"),
        0x19 => Some("p"),
        0x10 => Some("q"),
        0x13 => Some("r"),
        0x1F => Some("s"),
        0x14 => Some("t"),
        0x16 => Some("u"),
        0x2F => Some("v"),
        0x11 => Some("w"),
        0x2D => Some("x"),
        0x15 => Some("y"),
        0x2C => Some("z"),
        0x39 => Some(" "),
        0x0F => Some("\t"),
        0x1C => Some("\n"),
        _ => None,
    }
}
