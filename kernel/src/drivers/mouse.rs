use crate::drivers::framebuffer::Framebuffer;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Point, Primitive, RgbColor};
use embedded_graphics::primitives::{Polyline, PrimitiveStyle};

use serde::Serialize;
use x86_64::instructions::port::Port;

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

fn wait_input_ready() {
    while unsafe { Port::<u8>::new(0x64).read() } & 0x02 != 0 {}
}

fn wait_output_ready() {
    while unsafe { Port::<u8>::new(0x64).read() } & 0x01 == 0 {}
}
