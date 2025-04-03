use x86_64::instructions::port::Port;

#[derive(Default, Debug)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub altgr: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
    pub scroll_lock: bool,
}

pub struct Keyboard {
    data_port: Port<u8>,
    pub modifiers: Modifiers,
    extended: bool,
}

impl Keyboard {
    pub fn init() -> Self {
        Self {
            data_port: Port::new(0x60),
            modifiers: Modifiers::default(),
            extended: false,
        }
    }

    pub fn poll_key(&mut self) -> Option<u8> {
        let status: u8 = unsafe { Port::new(0x64).read() };
        if status & 0x01 == 0 {
            return None;
        }

        let scancode = unsafe { self.data_port.read() };

        if scancode == 0xE0 {
            self.extended = true;
            return None; // Wait for the next byte
        }

        let full_scancode = if self.extended {
            self.extended = false;
            0xE000 | scancode as u16 // Combine to make a full u16 extended code
        } else {
            scancode as u16
        };

        match full_scancode {
            0x2A | 0x36 => self.modifiers.shift = true,
            0xAA | 0xB6 => self.modifiers.shift = false,
            0x1D => self.modifiers.ctrl = true,
            0x9D => self.modifiers.ctrl = false,
            0x38 => self.modifiers.alt = true,
            0xB8 => self.modifiers.alt = false,
            0xE038 => self.modifiers.altgr = true,
            0xE0B8 => self.modifiers.altgr = false,
            0x3A => self.modifiers.caps_lock = !self.modifiers.caps_lock,
            0x45 => self.modifiers.num_lock = !self.modifiers.num_lock,
            0x46 => self.modifiers.scroll_lock = !self.modifiers.scroll_lock,
            _ => {}
        }

        Some(scancode)
    }
}
