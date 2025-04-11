use x86_64::instructions::port::Port;

use crate::framebuffer::Framebuffer;

pub struct Mouse {
    data_port: Port<u8>,
    cmd_port: Port<u8>,
    packet: [u8; 3],
    packet_index: usize,
    x: i32,
    y: i32,
    max_x: i32,
    max_y: i32,
}

impl Mouse {
    pub fn init(fb: &Framebuffer) -> Self {
        let mut cmd_port = Port::new(0x64);
        let mut data_port = Port::new(0x60);

        unsafe {
            // Enable auxiliary device (mouse)
            cmd_port.write(0xA8);
            Mouse::write_mouse_command(&mut cmd_port, &mut data_port, 0xF6); // default settings
            Mouse::write_mouse_command(&mut cmd_port, &mut data_port, 0xF4); // enable streaming
        }

        Self {
            data_port,
            cmd_port,
            packet: [0; 3],
            packet_index: 0,
            x: fb.width() as i32 / 2,
            y: fb.height() as i32 / 2,
            max_x: fb.width() as i32,
            max_y: fb.height() as i32,
        }
    }

    fn write_mouse_command(cmd_port: &mut Port<u8>, data_port: &mut Port<u8>, command: u8) {
        unsafe {
            cmd_port.write(0xD4); // tell PS/2 controller this is a mouse command
            data_port.write(command);
        }
    }

    pub fn poll(&mut self) -> Option<(i8, i8, u8)> {
        let status: u8 = unsafe { Port::new(0x64).read() };
        if status & 0x01 == 0 {
            return None;
        }

        let byte = unsafe { self.data_port.read() };

        match self.packet_index {
            0 => {
                if byte & 0x08 == 0 {
                    // Invalid first byte — drop
                    return None;
                }
            }
            _ => {}
        }

        self.packet[self.packet_index] = byte;
        self.packet_index += 1;

        if self.packet_index < 3 {
            return None;
        }

        self.packet_index = 0;
        let dx = self.packet[1] as i8;
        let dy = -(self.packet[2] as i8); // Y is inverted
        let buttons = self.packet[0] & 0b0000_0111;

        Some((dx, dy, buttons))
    }

    pub fn normalize_position(&self, dx: i8, dy: i8) -> (i32, i32) {
        let new_x = (self.x + dx as i32).clamp(0, self.max_x - 1);
        let new_y = (self.y + dy as i32).clamp(0, self.max_y - 1);
        (new_x, new_y)
    }

    pub fn update_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }
}
