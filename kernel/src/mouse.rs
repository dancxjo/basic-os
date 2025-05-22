use crate::framebuffer::Framebuffer;
use embedded_graphics::pixelcolor::Rgb565;
use log::{debug, error, warn};
use serde::Serialize;
use x86_64::{instructions::port::Port, structures::paging::frame};

#[derive(Clone, Serialize)]
pub struct Mouse {
    pub x: usize,
    pub y: usize,
    prev_x: usize,
    prev_y: usize,
    screen_width: usize,
    screen_height: usize,
    dirty: bool,
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

        let new_x = (self.x as isize)
            .saturating_add(dx)
            .clamp(0, self.screen_width as isize - 1);
        let new_y = (self.y as isize)
            .saturating_add(dy)
            .clamp(0, self.screen_height as isize - 1);

        self.x = new_x as usize;
        self.y = new_y as usize;
        self.dirty = true;
    }

    pub fn position(&self) -> (usize, usize) {
        (self.x, self.y)
    }

    pub fn draw(&self, framebuffer: &mut Framebuffer) {
        let cursor_color = Rgb565::new(0, 0, 255);
        let encode_color = framebuffer.encode_color_rgb565(cursor_color);

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

        // Clamp erase area to screen edge
        let x0 = self.prev_x.min(framebuffer.width.saturating_sub(5));
        let y0 = self.prev_y.min(framebuffer.height.saturating_sub(5));
        framebuffer.erase_region(x0, y0, 5, 5);

        let width = framebuffer.width;
        let height = framebuffer.height;
        let pitch_pixels = framebuffer.pitch_pixels;
        let direct_fb = framebuffer.dangerous_direct_access_mut();

        for (dx, dy) in cursor_shape.iter() {
            if let (Some(px), Some(py)) = (self.x.checked_add(*dx), self.y.checked_add(*dy)) {
                if px < width && py < height {
                    let index = py * pitch_pixels + px;
                    if index < direct_fb.len() {
                        if index >= direct_fb.len() {
                            error!(
                                "Framebuffer index OOB: index={} (max={})",
                                index,
                                direct_fb.len()
                            );
                            return;
                        }

                        direct_fb[index] = encode_color;
                    }
                }
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
}

fn wait_input_ready() {
    while unsafe { Port::<u8>::new(0x64).read() } & 0x02 != 0 {}
}

fn wait_output_ready() {
    while unsafe { Port::<u8>::new(0x64).read() } & 0x01 == 0 {}
}
