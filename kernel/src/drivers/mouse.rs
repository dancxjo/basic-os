use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::drivers::framebuffer::Framebuffer;
use crate::drivers::input::InputBuffer;
use crate::drivers::registry::{DriverDescriptor, DriverKind};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Point, Primitive, RgbColor};
use embedded_graphics::primitives::{Polyline, PrimitiveStyle};

use log::warn;
use serde::Serialize;
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

const MOUSE_RAW_CAPACITY: usize = 256;
const MOUSE_EVENT_CAPACITY: usize = 64;

pub static MOUSE_RAW_BYTES: InputBuffer<u8, MOUSE_RAW_CAPACITY> = InputBuffer::new(0);
pub static MOUSE_EVENTS: InputBuffer<MouseEvent, MOUSE_EVENT_CAPACITY> =
    InputBuffer::new(MouseEvent::default());
pub const DRIVER: DriverDescriptor = DriverDescriptor::new(
    "ps2-mouse",
    DriverKind::Input,
    "PS/2 mouse (3-byte packet)",
    init,
);

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct MouseEvent {
    pub dx: i8,
    pub dy: i8,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

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
        // Resynchronize if the first byte does not contain the always-set bit 3.
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
        let dy = (self.packet[2] as i8).wrapping_neg(); // PS/2 Y grows upwards

        if flags & 0x40 != 0 || flags & 0x80 != 0 {
            // Drop overflow packets.
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

#[derive(Clone, Serialize)]
pub struct Mouse {
    pub x: usize,
    pub y: usize,
    pub screen_width: usize,
    pub screen_height: usize,
    pub dirty: bool,
    pub prev_x: usize,
    pub prev_y: usize,
}

impl Mouse {
    pub fn new(fb: &Framebuffer) -> Self {
        Mouse {
            x: fb.width / 2,
            y: fb.height / 2,
            screen_width: fb.width,
            screen_height: fb.height,
            dirty: false,
            prev_x: fb.width / 2,
            prev_y: fb.height / 2,
        }
    }

    pub fn move_by(&mut self, dx: isize, dy: isize) {
        self.prev_x = self.x;
        self.prev_y = self.y;

        let new_x = (self.x as isize + dx).clamp(0, self.screen_width as isize - 1);
        let new_y = (self.y as isize + dy).clamp(0, self.screen_height as isize - 1);

        self.x = new_x as usize;
        self.y = new_y as usize;
        self.dirty = true;
    }

    pub fn position(&self) -> (usize, usize) {
        (self.x, self.y)
    }

    pub fn draw(&self, _framebuffer: &mut Framebuffer) {
        let origin = Point::new(self.x as i32, self.y as i32);

        // Shrunk shape (scaled by ~0.4x from the original)
        let points = [
            Point::new(0, 0), // tip
            Point::new(0, 40),
            Point::new(12, 28),
            Point::new(20, 48),
            Point::new(24, 44),
            Point::new(16, 24),
            Point::new(28, 24),
            Point::new(0, 0), // close
        ]
        .map(|p| p + origin);

        let _arrow =
            Polyline::new(&points).into_styled(PrimitiveStyle::with_stroke(Rgb565::BLACK, 2));

        // let _ = arrow.draw(framebuffer);
    }
}

/// Retrieve the next decoded mouse event, if any.
pub fn pop_event() -> Option<MouseEvent> {
    MOUSE_EVENTS.pop()
}

/// Low-level IRQ handler. Keeps raw bytes for debugging and emits
/// high-level `MouseEvent` records so higher layers stay decoupled from PS/2.
pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut data_port = Port::<u8>::new(0x60);
    let packet: u8 = unsafe { data_port.read() };

    if MOUSE_RAW_BYTES.push(packet).is_err() {
        warn!("Mouse packet buffer overflow");
    }

    let mut decoder = MOUSE_PACKET_DECODER.lock();
    if let Some(event) = decoder.feed(packet) {
        if MOUSE_EVENTS.push(event).is_err() {
            warn!("Mouse event buffer overflow");
        }
    }

    end_of_interrupt(12);
}

static MOUSE_PACKET_DECODER: Mutex<PacketDecoder> = Mutex::new(PacketDecoder::new());

pub fn init() -> Result<(), &'static str> {
    enable_irq();
    Ok(())
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
