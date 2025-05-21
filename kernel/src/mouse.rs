use crate::framebuffer::Framebuffer;
use embedded_graphics::pixelcolor::Rgb565;
use log::debug;
use serde::Serialize;
use x86_64::{instructions::port::Port, structures::paging::frame};

#[derive(Clone, Serialize)]
pub struct Mouse {
    pub x: usize,
    pub y: usize,
    screen_width: usize,
    screen_height: usize,
    dirty: bool,
}

impl Mouse {
    pub fn new(fb: &Framebuffer) -> Self {
        unsafe {
            Port::<u8>::new(0x64).write(0xA8); // Enable auxiliary device
            Port::<u8>::new(0x64).write(0x20); // Read command byte
            while (Port::<u8>::new(0x64).read() & 1) == 0 {}
            let status = Port::<u8>::new(0x60).read();
            Port::<u8>::new(0x64).write(0x60); // Write command byte
            Port::<u8>::new(0x60).write(status | 2);
            Port::<u8>::new(0x64).write(0xD4);
            Port::<u8>::new(0x60).write(0xF4); // Enable data reporting
            Port::<u8>::new(0x60).read(); // ACK
        }

        Mouse {
            x: fb.width / 2,
            y: fb.height / 2,
            screen_width: fb.width,
            screen_height: fb.height,
            dirty: false,
        }
    }

    pub fn move_by(&mut self, dx: isize, dy: isize) {
        self.x = ((self.x as isize + dx).clamp(0, self.screen_width as isize - 1)) as usize;
        self.y = ((self.y as isize + dy).clamp(0, self.screen_height as isize - 1)) as usize;
        self.dirty = true;
    }

    pub fn position(&self) -> (usize, usize) {
        (self.x, self.y)
    }

    pub fn draw(&self, framebuffer: &mut Framebuffer) {
        let (x, y) = self.position();
        let cursor_color = Rgb565::new(0, 0, 255);
        let background = Rgb565::new(250 >> 3, 250 >> 2, 245 >> 3);

        // Erase the previous pointer
        framebuffer.erase_region(x, y, 5, 5);

        // Draw the new pointer
        let cursor_shape = [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 0),
            (0, 1),
            (4, 1),
            (0, 2),
            (4, 2),
            (0, 3),
            (4, 3),
            (1, 4),
            (2, 4),
            (3, 4),
        ];

        let width = framebuffer.width;
        let height = framebuffer.height;
        let pitch_pixels = framebuffer.pitch_pixels;
        let encode_color = framebuffer.encode_color_rgb565(cursor_color);

        let direct_fb = framebuffer.dangerous_direct_access_mut();
        for (dx, dy) in cursor_shape.iter() {
            let px = x + dx;
            let py = y + dy;
            if px < width && py < height {
                let index = py * pitch_pixels + px;
                direct_fb[index] = encode_color;
            }
        }
    }

    pub fn needs_update(&mut self) -> bool {
        if self.dirty {
            self.dirty = false;
            true
        } else {
            false
        }
    }

    pub fn poll(&mut self) {
        static mut BYTE_IDX: u8 = 0;
        static mut PACKET: [u8; 3] = [0; 3];

        unsafe {
            let status = Port::<u8>::new(0x64).read();
            if status & 1 == 0 {
                return;
            }

            let data = Port::<u8>::new(0x60).read();
            PACKET[BYTE_IDX as usize] = data;
            BYTE_IDX += 1;

            if BYTE_IDX >= 3 {
                BYTE_IDX = 0;
                let dx = PACKET[1] as i8 as isize;
                let dy = -(PACKET[2] as i8 as isize);
                self.move_by(dx, dy);
                debug!("[poll] Mouse moved to ({}, {})", self.x, self.y);
            }
        }
    }
}
