use crate::{beat::Beat, framebuffer::Framebuffer, space::Space, thing::Fact};
use alloc::vec;
use alloc::{format, vec::Vec};
use log::debug;
use serde::Serialize;
use uuid::Uuid;
use x86_64::instructions::port::Port;

#[derive(Clone, Serialize)]
pub struct Mouse {
    pub x: usize,
    pub y: usize,
    screen_width: usize,
    screen_height: usize,
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
        }
    }

    pub fn move_by(&mut self, dx: isize, dy: isize) {
        self.x = ((self.x as isize + dx).clamp(0, self.screen_width as isize - 1)) as usize;
        self.y = ((self.y as isize + dy).clamp(0, self.screen_height as isize - 1)) as usize;
    }

    pub fn position(&self) -> (usize, usize) {
        (self.x, self.y)
    }

    pub fn draw(&self, _fb: &mut Framebuffer) {
        let (_x, _y) = self.position();
        // fb.draw_circle(x, y, 5, 0xFF0000);
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

impl Beat for Mouse {
    fn beat(&mut self, self_id: Uuid, _space: &Space) -> Vec<Fact> {
        self.poll();

        let moved_to = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"moved_to");

        let pos_string = format!("{},{}", self.x, self.y);
        let pos_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, pos_string.as_bytes());

        vec![Fact::new(self_id, moved_to, pos_id, false)]
    }
}
