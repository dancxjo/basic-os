use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::drivers::device::{self, DeviceKind, KEYBOARD_DEVICE_NAME};
use crate::drivers::input::InputBuffer;
use crate::drivers::irq_dma;
use crate::graph::{
    self, GraphFiatRequest, QueueStateSpec, SharedBufferSpec, canon, journal::Value,
};
use alloc::collections::BTreeMap;
use log::warn;
use spin::Mutex as SpinMutex;
use uuid::Uuid;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub const KEYBOARD_BUFFER_LEN: usize = 256;

pub static KEYBOARD_BUFFER: InputBuffer<u8, KEYBOARD_BUFFER_LEN> = InputBuffer::new(0);

#[derive(Clone, Copy)]
struct KeyboardBufferGraph {
    buffer_id: Uuid,
    queue_state_id: Uuid,
}

static KEYBOARD_BUFFER_GRAPH: SpinMutex<Option<KeyboardBufferGraph>> = SpinMutex::new(None);

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
    refresh_keyboard_queue_state();
    written
}

fn publish_keyboard_buffer_nodes() {
    let mut buffer_props = BTreeMap::new();
    buffer_props.insert(canon::NAME, Value::Text("kbd.ps2.rx".into()));
    buffer_props.insert(canon::STATUS, Value::Symbol(canon::INIT));

    let shared_buffer = graph::declare_shared_buffer(
        graph::KERNEL_BUNDLE_ID,
        SharedBufferSpec {
            id: None,
            size_bytes: KEYBOARD_BUFFER_LEN as u64,
            kind: canon::RING,
            usage: canon::RX_RING_USAGE,
            addr: None,
            props: buffer_props,
        },
    );

    let (head, tail) = KEYBOARD_BUFFER.positions();
    let queue_state = graph::declare_queue_state(QueueStateSpec {
        id: None,
        buffer: shared_buffer.id,
        owner: graph::KERNEL_BUNDLE_ID,
        head: head as u64,
        tail: tail as u64,
        has_data: false,
        capacity: Some(KEYBOARD_BUFFER_LEN as u64),
        props: BTreeMap::new(),
    });

    *KEYBOARD_BUFFER_GRAPH.lock() = Some(KeyboardBufferGraph {
        buffer_id: shared_buffer.id,
        queue_state_id: queue_state.id,
    });
}

fn refresh_keyboard_queue_state() {
    let (head, tail) = KEYBOARD_BUFFER.positions();
    let len = KEYBOARD_BUFFER.len();
    let capacity = KEYBOARD_BUFFER_LEN as u64;
    let queue_id = {
        let guard = KEYBOARD_BUFFER_GRAPH.lock();
        guard.as_ref().map(|state| state.queue_state_id)
    };
    if let Some(queue_state_id) = queue_id {
        let _ = graph::update_queue_state(
            graph::KERNEL_BUNDLE_ID,
            queue_state_id,
            head as u64,
            tail as u64,
            len > 0,
            Some(capacity),
        );
    }
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
    publish_keyboard_buffer_nodes();
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

pub fn process_events() {
    // crate::serial_println!("KBD: process_events start");

    loop {
        let scancode = KEYBOARD_BUFFER.pop();
        match scancode {
            Some(c) => {
                // crate::serial_println!("KBD: got scancode {:02x}", c);
                irq_dma::for_each_binding(1, |binding| {
                    emit_key_event(c, binding);
                });
                // crate::serial_println!("KBD: skipped emit");
            }
            None => break,
        }
    }
    refresh_keyboard_queue_state();
    // crate::serial_println!("KBD: process_events end");
}

fn emit_key_event(scancode: u8, binding: &irq_dma::IrqBindingInfo) {
    let device = keyboard_device_id();
    if binding.device != device {
        return;
    }

    let (code, down) = decode_scancode(scancode);
    let key_text = scancode_text(code);
    let ts = irq_dma::monotonic_ticks();

    let mut fields = BTreeMap::new();
    fields.insert(canon::DEVICE_ID, Value::Uuid(binding.device));
    fields.insert(canon::SCANCODE, Value::U64(code as u64));
    fields.insert(canon::DOWN, Value::Bool(down));
    fields.insert(canon::TS, Value::U64(ts));
    // if let Some(text) = key_text {
    //     fields.insert(canon::TEXT, Value::Text(text.into()));
    // }

    let req = GraphFiatRequest {
        id: None,
        kind: canon::KEY_EVENT,
        fields,
    };

    let _ = graph::fiat_for_bundle(binding.bundle, req);
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
    refresh_keyboard_queue_state();
    written
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
