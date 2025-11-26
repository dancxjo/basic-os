//! Userland driver runtime: wraps device syscalls and hosts driver instances
//! that decode raw device data into higher-level events.

use alloc::vec::Vec;

use crate::canon;
use crate::graph::{map, Value};
use crate::sys;
use crate::Symbol;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DeviceKind {
    Keyboard = 1,
    Mouse = 2,
    Framebuffer = 3,
    Serial = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceHandle(pub u64);

pub trait Driver {
    fn poll(&mut self, ctx: &mut DriverContext);
}

pub struct DriverContext {
    handles: Vec<DeviceHandle>,
}

impl DriverContext {
    pub fn new() -> Self {
        Self {
            handles: Vec::new(),
        }
    }

    pub fn open_device(&mut self, kind: DeviceKind, index: u32) -> Option<DeviceHandle> {
        let handle = sys::dev_open(kind as u32, index)?;
        let handle = DeviceHandle(handle);
        self.handles.push(handle);
        Some(handle)
    }

    pub fn read_device(&self, handle: DeviceHandle, out: &mut [u8]) -> usize {
        sys::dev_read(handle.0, out)
    }

    pub fn write_device(&self, handle: DeviceHandle, buf: &[u8]) -> usize {
        sys::dev_write(handle.0, buf)
    }

    pub fn map_device(&self, handle: DeviceHandle) -> Option<sys::DeviceMapping> {
        sys::dev_map(handle.0)
    }

    pub fn emit_event(&self, kind: Symbol, data: Value) {
        if let Ok(buf) = postcard::to_allocvec(&data) {
            let _ = sys::journal_emit_raw(kind.0, &buf);
        }
    }
}

// ------------------ Keyboard driver ------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeadKey {
    None,
    Acute,
    Grave,
    Circumflex,
    Diaeresis,
    Tilde,
}

#[derive(Clone, Copy)]
enum KeyCode {
    Printable(char),
    Backspace,
    Tab,
    Enter,
    Escape,
    Space,
    Function(u8),
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    YieldNow,
    Unknown(u8),
}

#[derive(Clone, Copy)]
struct KeyEvent {
    code: KeyCode,
    pressed: bool,
    scancode: u8,
}

pub struct KeyboardDriver {
    dev: DeviceHandle,
    shift: bool,
    caps_lock: bool,
    altgr: bool,
    deadkey: DeadKey,
    prefix: u8,
}

impl KeyboardDriver {
    pub fn init(ctx: &mut DriverContext) -> Option<Self> {
        let dev = ctx.open_device(DeviceKind::Keyboard, 0)?;
        Some(Self {
            dev,
            shift: false,
            caps_lock: false,
            altgr: false,
            deadkey: DeadKey::None,
            prefix: 0,
        })
    }

    pub fn poll(&mut self, ctx: &mut DriverContext) {
        let mut buf = [0u8; 64];
        let count = ctx.read_device(self.dev, &mut buf);
        for scancode in buf.iter().copied().take(count) {
            self.process_scancode(scancode, ctx);
        }
    }

    fn process_scancode(&mut self, scancode: u8, ctx: &mut DriverContext) {
        if self.prefix == 0xE0 {
            self.prefix = 0;

            if self.update_modifier_extended(scancode) {
                return;
            }

            if let Some(event) = self.decode_extended(scancode) {
                self.handle_key_event(event, ctx);
            }
            return;
        }

        if scancode == 0xE0 {
            self.prefix = 0xE0;
            return;
        }

        if self.update_modifier(scancode) {
            return;
        }

        if let Some(event) = self.decode_basic(scancode) {
            self.handle_key_event(event, ctx);
        }
    }

    fn update_modifier(&mut self, scancode: u8) -> bool {
        let released = scancode & 0x80 != 0;
        let code = scancode & 0x7F;

        match code {
            0x2A | 0x36 => {
                self.shift = !released;
                true
            }
            0x38 => {
                self.altgr = !released;
                true
            }
            0x3A if !released => {
                self.caps_lock = !self.caps_lock;
                true
            }
            _ => false,
        }
    }

    fn update_modifier_extended(&mut self, scancode: u8) -> bool {
        let released = scancode & 0x80 != 0;

        match scancode {
            0x38 | 0xB8 => {
                self.altgr = !released;
                true
            }
            _ => false,
        }
    }

    fn start_dead_key(&mut self, scancode: u8) -> bool {
        match scancode {
            0x28 => {
                self.deadkey = DeadKey::Acute;
                true
            }
            0x29 => {
                self.deadkey = if self.shift {
                    DeadKey::Tilde
                } else {
                    DeadKey::Grave
                };
                true
            }
            0x2B => {
                self.deadkey = DeadKey::Circumflex;
                true
            }
            0x1A => {
                self.deadkey = DeadKey::Diaeresis;
                true
            }
            _ => false,
        }
    }

    fn decode_basic(&mut self, scancode: u8) -> Option<KeyEvent> {
        let pressed = scancode & 0x80 == 0;
        let code = scancode & 0x7F;

        let key = match code {
            0x01 => KeyCode::Escape,
            0x0F => KeyCode::Tab,
            0x1C => KeyCode::Enter,
            0x39 => KeyCode::Space,
            0x0E => KeyCode::Backspace,
            0x0F | 0x38 => KeyCode::YieldNow,
            0x3B..=0x44 => KeyCode::Function(code as u8 - 0x3A),
            0x48 => KeyCode::ArrowUp,
            0x50 => KeyCode::ArrowDown,
            0x4B => KeyCode::ArrowLeft,
            0x4D => KeyCode::ArrowRight,
            _ => {
                if self.start_dead_key(code) {
                    return None;
                }
                return self.scancode_to_char(code).map(|c| KeyEvent {
                    code: KeyCode::Printable(c),
                    pressed,
                    scancode: code,
                });
            }
        };

        Some(KeyEvent {
            code: key,
            pressed,
            scancode: code,
        })
    }

    fn decode_extended(&self, scancode: u8) -> Option<KeyEvent> {
        let pressed = scancode & 0x80 == 0;
        let code = scancode & 0x7F;

        let key = match code {
            0x1C => KeyCode::Enter, // Keypad Enter
            0x48 => KeyCode::ArrowUp,
            0x50 => KeyCode::ArrowDown,
            0x4B => KeyCode::ArrowLeft,
            0x4D => KeyCode::ArrowRight,
            0x47 => KeyCode::Home,
            0x49 => KeyCode::PageUp,
            0x4F => KeyCode::End,
            0x51 => KeyCode::PageDown,
            0x52 => KeyCode::Insert,
            0x53 => KeyCode::Delete,
            _ => return None,
        };

        Some(KeyEvent {
            code: key,
            pressed,
            scancode: code,
        })
    }

    fn handle_key_event(&mut self, event: KeyEvent, ctx: &mut DriverContext) {
        if !event.pressed {
            return;
        }

        let mut payload = map();
        payload.insert(canon::SCANCODE, Value::U64(event.scancode as u64));
        if let Some(symbol) = key_symbol(event.code) {
            payload.insert(canon::KEY, Value::Symbol(symbol));
        }
        let ev = Value::Map(payload);
        ctx.emit_event(canon::KEY_PRESSED, ev);
    }

    fn scancode_to_char(&mut self, scancode: u8) -> Option<char> {
        let uppercase = self.shift ^ self.caps_lock;

        let ch = if let Some(letter) = letter_from_scancode(scancode) {
            if uppercase {
                letter.to_ascii_uppercase()
            } else {
                letter
            }
        } else if let Some(digit) = digit_from_scancode(scancode, self.shift) {
            digit
        } else {
            symbol_from_scancode(scancode, self.shift, self.altgr)?
        };

        let dead = self.deadkey;
        self.deadkey = DeadKey::None;
        if let Some(composed) = apply_dead_key(dead, ch) {
            return Some(composed);
        }
        Some(ch)
    }
}

fn letter_from_scancode(scancode: u8) -> Option<char> {
    let letter = match scancode {
        0x10 => 'q',
        0x11 => 'w',
        0x12 => 'e',
        0x13 => 'r',
        0x14 => 't',
        0x15 => 'y',
        0x16 => 'u',
        0x17 => 'i',
        0x18 => 'o',
        0x19 => 'p',
        0x1E => 'a',
        0x1F => 's',
        0x20 => 'd',
        0x21 => 'f',
        0x22 => 'g',
        0x23 => 'h',
        0x24 => 'j',
        0x25 => 'k',
        0x26 => 'l',
        0x2C => 'z',
        0x2D => 'x',
        0x2E => 'c',
        0x2F => 'v',
        0x30 => 'b',
        0x31 => 'n',
        0x32 => 'm',
        _ => return None,
    };
    Some(letter)
}

fn digit_from_scancode(scancode: u8, shift: bool) -> Option<char> {
    Some(match scancode {
        0x0B => '0',
        0x02 => '1',
        0x03 => '2',
        0x04 => '3',
        0x05 => '4',
        0x06 => '5',
        0x07 => '6',
        0x08 => '7',
        0x09 => '8',
        0x0A => '9',
        _ => return None,
    })
    .map(|c| {
        if shift {
            match c {
                '1' => '!',
                '2' => '@',
                '3' => '#',
                '4' => '$',
                '5' => '%',
                '6' => '^',
                '7' => '&',
                '8' => '*',
                '9' => '(',
                '0' => ')',
                other => other,
            }
        } else {
            c
        }
    })
}

fn symbol_from_scancode(scancode: u8, shift: bool, altgr: bool) -> Option<char> {
    let symbol = match scancode {
        0x1A => {
            if altgr {
                '}'
            } else {
                return None;
            }
        }
        0x1B => '[',
        0x27 => ';',
        0x28 => '\'',
        0x29 => '`',
        0x2B => '\\',
        0x33 => ',',
        0x34 => '.',
        0x35 => '/',
        0x56 => {
            if altgr {
                '\\'
            } else {
                '`'
            }
        }
        0x73 => ';',
        0x7D => {
            if altgr {
                '|'
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some(if shift {
        match symbol {
            '[' => '{',
            ']' => '}',
            ';' => ':',
            '\'' => '"',
            '`' => '~',
            '\\' => '|',
            ',' => '<',
            '.' => '>',
            '/' => '?',
            c => c,
        }
    } else {
        symbol
    })
}

fn apply_dead_key(dead: DeadKey, ch: char) -> Option<char> {
    let lower = ch.to_ascii_lowercase();

    let composed = match (dead, lower) {
        (DeadKey::Acute, 'a') => Some('á'),
        (DeadKey::Acute, 'e') => Some('é'),
        (DeadKey::Acute, 'i') => Some('í'),
        (DeadKey::Acute, 'o') => Some('ó'),
        (DeadKey::Acute, 'u') => Some('ú'),
        (DeadKey::Acute, 'c') => Some('ć'),
        (DeadKey::Acute, 'n') => Some('ń'),

        (DeadKey::Grave, 'a') => Some('à'),
        (DeadKey::Grave, 'e') => Some('è'),
        (DeadKey::Grave, 'i') => Some('ì'),
        (DeadKey::Grave, 'o') => Some('ò'),
        (DeadKey::Grave, 'u') => Some('ù'),

        (DeadKey::Circumflex, 'a') => Some('â'),
        (DeadKey::Circumflex, 'e') => Some('ê'),
        (DeadKey::Circumflex, 'i') => Some('î'),
        (DeadKey::Circumflex, 'o') => Some('ô'),
        (DeadKey::Circumflex, 'u') => Some('û'),
        (DeadKey::Circumflex, 'c') => Some('ĉ'),

        (DeadKey::Diaeresis, 'a') => Some('ä'),
        (DeadKey::Diaeresis, 'e') => Some('ë'),
        (DeadKey::Diaeresis, 'i') => Some('ï'),
        (DeadKey::Diaeresis, 'o') => Some('ö'),
        (DeadKey::Diaeresis, 'u') => Some('ü'),
        (DeadKey::Diaeresis, 'y') => Some('ÿ'),

        (DeadKey::Tilde, 'a') => Some('ã'),
        (DeadKey::Tilde, 'o') => Some('õ'),
        (DeadKey::Tilde, 'n') => Some('ñ'),

        _ => None,
    }?;

    if ch.is_uppercase() {
        composed.to_uppercase().next()
    } else {
        Some(composed)
    }
}

fn key_symbol(code: KeyCode) -> Option<Symbol> {
    Some(match code {
        KeyCode::Printable(c) => canon::from_char(c),
        KeyCode::Tab => canon::cc('T', 'B'),
        KeyCode::Enter => canon::cc('E', 'N'),
        KeyCode::Escape => canon::cc('E', 'S'),
        KeyCode::Space => canon::from_char(' '),
        KeyCode::Function(idx) => Symbol::new(0xF000 | idx as u32),
        KeyCode::ArrowUp => canon::cc('A', 'U'),
        KeyCode::ArrowDown => canon::cc('A', 'D'),
        KeyCode::ArrowLeft => canon::cc('A', 'L'),
        KeyCode::ArrowRight => canon::cc('A', 'R'),
        KeyCode::Home => canon::cc('H', 'M'),
        KeyCode::End => canon::cc('E', 'D'),
        KeyCode::PageUp => canon::cc('P', 'U'),
        KeyCode::PageDown => canon::cc('P', 'D'),
        KeyCode::Insert => canon::cc('I', 'N'),
        KeyCode::Delete => canon::cc('D', 'L'),
        KeyCode::YieldNow => canon::cc('Y', 'L'),
        KeyCode::Backspace => canon::cc('B', 'S'),
        KeyCode::Unknown(_) => return None,
    })
}

// ------------------ Mouse driver ------------------

#[derive(Clone, Copy, Debug, Default)]
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

        Some(MouseEvent {
            dx,
            dy,
            left: flags & 0x01 != 0,
            right: flags & 0x02 != 0,
            middle: flags & 0x04 != 0,
        })
    }
}

pub struct MouseDriver {
    dev: DeviceHandle,
    decoder: PacketDecoder,
}

impl MouseDriver {
    pub fn init(ctx: &mut DriverContext) -> Option<Self> {
        let dev = ctx.open_device(DeviceKind::Mouse, 0)?;
        Some(Self {
            dev,
            decoder: PacketDecoder::new(),
        })
    }

    pub fn poll(&mut self, ctx: &mut DriverContext) {
        let mut buf = [0u8; 64];
        let count = ctx.read_device(self.dev, &mut buf);
        for byte in buf.iter().copied().take(count) {
            if let Some(event) = self.decoder.feed(byte) {
                self.emit_event(event, ctx);
            }
        }
    }

    fn emit_event(&self, event: MouseEvent, ctx: &mut DriverContext) {
        let mut payload = map();
        payload.insert(canon::DX, Value::I64(event.dx as i64));
        payload.insert(canon::DY, Value::I64(event.dy as i64));
        let buttons: u8 =
            (event.left as u8) | ((event.right as u8) << 1) | ((event.middle as u8) << 2);
        payload.insert(canon::BUTTONS, Value::U64(buttons as u64));
        ctx.emit_event(canon::MOUSE_MOVED, Value::Map(payload));
    }
}

// ------------------ Framebuffer driver ------------------

pub struct FramebufferDriver {
    dev: DeviceHandle,
    pub mapping: Option<sys::DeviceMapping>,
}

impl FramebufferDriver {
    pub fn init(ctx: &mut DriverContext) -> Option<Self> {
        let dev = ctx.open_device(DeviceKind::Framebuffer, 0)?;
        let mapping = ctx.map_device(dev);
        Some(Self { dev, mapping })
    }

    pub fn blit(&self, ctx: &DriverContext, data: &[u8]) -> usize {
        ctx.write_device(self.dev, data)
    }
}

impl Driver for FramebufferDriver {
    fn poll(&mut self, _ctx: &mut DriverContext) {}
}

// ------------------ Serial driver ------------------

pub struct SerialDriver {
    dev: DeviceHandle,
}

impl SerialDriver {
    pub fn init(ctx: &mut DriverContext) -> Option<Self> {
        let dev = ctx.open_device(DeviceKind::Serial, 0)?;
        Some(Self { dev })
    }

    pub fn write(&self, ctx: &DriverContext, data: &[u8]) -> usize {
        ctx.write_device(self.dev, data)
    }
}

impl Driver for SerialDriver {
    fn poll(&mut self, _ctx: &mut DriverContext) {}
}

pub fn default_drivers(ctx: &mut DriverContext) -> (Option<KeyboardDriver>, Option<MouseDriver>) {
    let keyboard = KeyboardDriver::init(ctx);
    let mouse = MouseDriver::init(ctx);
    (keyboard, mouse)
}
