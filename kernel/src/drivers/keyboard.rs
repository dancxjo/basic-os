use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::serial_print;
use crate::system::SYSTEM;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

pub static SELECTED_SCREEN: AtomicUsize = AtomicUsize::new(1); // default to Screen 1

/// Modifier state
static SHIFT: AtomicBool = AtomicBool::new(false);
static ALTGR: AtomicBool = AtomicBool::new(false);
static DEADKEY: AtomicUsize = AtomicUsize::new(0);
static PREFIX: AtomicU8 = AtomicU8::new(0);

#[derive(PartialEq)]
enum DeadKey {
    None,
    Acute,
    Grave,
    Circumflex,
    Diaeresis,
    Tilde,
}

impl DeadKey {
    fn from_usize(value: usize) -> Self {
        match value {
            1 => DeadKey::Acute,
            2 => DeadKey::Grave,
            3 => DeadKey::Circumflex,
            4 => DeadKey::Diaeresis,
            5 => DeadKey::Tilde,
            _ => DeadKey::None,
        }
    }

    fn to_usize(&self) -> usize {
        match self {
            DeadKey::Acute => 1,
            DeadKey::Grave => 2,
            DeadKey::Circumflex => 3,
            DeadKey::Diaeresis => 4,
            DeadKey::Tilde => 5,
            DeadKey::None => 0,
        }
    }
}

pub fn scancode_to_char(scancode: u8) -> Option<char> {
    let shift = SHIFT.load(Ordering::Relaxed);
    let deadkey = DeadKey::from_usize(DEADKEY.load(Ordering::Relaxed));

    if deadkey == DeadKey::None {
        match scancode {
            0x28 => {
                DEADKEY.store(1, Ordering::Relaxed);
                return None;
            }
            0x29 => {
                DEADKEY.store(if shift { 5 } else { 2 }, Ordering::Relaxed);
                return None;
            }
            0x2B => {
                DEADKEY.store(3, Ordering::Relaxed);
                return None;
            }
            0x1A => {
                DEADKEY.store(4, Ordering::Relaxed);
                return None;
            }
            _ => {}
        }
    }

    let result = match (deadkey, scancode) {
        (DeadKey::Acute, 0x10) => Some('á'),
        (DeadKey::Acute, 0x12) => Some('é'),
        (DeadKey::Acute, 0x17) => Some('í'),
        (DeadKey::Acute, 0x18) => Some('ó'),
        (DeadKey::Acute, 0x16) => Some('ú'),
        (DeadKey::Acute, 0x1E) => Some('á'),
        (DeadKey::Acute, 0x20) => Some('é'),
        (DeadKey::Acute, 0x22) => Some('í'),
        (DeadKey::Acute, 0x2C) => Some('ó'),
        (DeadKey::Acute, 0x2E) => Some('ć'),
        (DeadKey::Acute, 0x31) => Some('ń'),

        (DeadKey::Grave, 0x10) => Some('à'),
        (DeadKey::Grave, 0x12) => Some('è'),
        (DeadKey::Grave, 0x17) => Some('ì'),
        (DeadKey::Grave, 0x18) => Some('ò'),
        (DeadKey::Grave, 0x16) => Some('ù'),
        (DeadKey::Grave, 0x1E) => Some('à'),

        (DeadKey::Circumflex, 0x10) => Some('â'),
        (DeadKey::Circumflex, 0x12) => Some('ê'),
        (DeadKey::Circumflex, 0x17) => Some('î'),
        (DeadKey::Circumflex, 0x18) => Some('ô'),
        (DeadKey::Circumflex, 0x16) => Some('û'),
        (DeadKey::Circumflex, 0x1E) => Some('â'),
        (DeadKey::Circumflex, 0x2E) => Some('ĉ'),

        (DeadKey::Diaeresis, 0x10) => Some('ä'),
        (DeadKey::Diaeresis, 0x12) => Some('ë'),
        (DeadKey::Diaeresis, 0x17) => Some('ï'),
        (DeadKey::Diaeresis, 0x18) => Some('ö'),
        (DeadKey::Diaeresis, 0x16) => Some('ü'),
        (DeadKey::Diaeresis, 0x1E) => Some('ä'),
        (DeadKey::Diaeresis, 0x31) => Some('ÿ'),

        (DeadKey::Tilde, 0x10) => Some('ã'),
        (DeadKey::Tilde, 0x18) => Some('õ'),
        (DeadKey::Tilde, 0x1E) => Some('ã'),
        (DeadKey::Tilde, 0x31) => Some('ñ'),

        _ => match (shift, scancode) {
            (false, 0x02..=0x0B) => Some("1234567890".chars().nth((scancode - 0x02) as usize)?),
            (true, 0x02..=0x0B) => Some("!@#$%^&*()".chars().nth((scancode - 0x02) as usize)?),

            (false, 0x10) => Some('q'),
            (true, 0x10) => Some('Q'),
            (false, 0x11) => Some('w'),
            (true, 0x11) => Some('W'),
            (false, 0x12) => Some('e'),
            (true, 0x12) => Some('E'),
            (false, 0x13) => Some('r'),
            (true, 0x13) => Some('R'),
            (false, 0x14) => Some('t'),
            (true, 0x14) => Some('T'),
            (false, 0x15) => Some('y'),
            (true, 0x15) => Some('Y'),
            (false, 0x16) => Some('u'),
            (true, 0x16) => Some('U'),
            (false, 0x17) => Some('i'),
            (true, 0x17) => Some('I'),
            (false, 0x18) => Some('o'),
            (true, 0x18) => Some('O'),
            (false, 0x19) => Some('p'),
            (true, 0x19) => Some('P'),

            (false, 0x1E) => Some('a'),
            (true, 0x1E) => Some('A'),
            (false, 0x1F) => Some('s'),
            (true, 0x1F) => Some('S'),
            (false, 0x20) => Some('d'),
            (true, 0x20) => Some('D'),
            (false, 0x21) => Some('f'),
            (true, 0x21) => Some('F'),
            (false, 0x22) => Some('g'),
            (true, 0x22) => Some('G'),
            (false, 0x23) => Some('h'),
            (true, 0x23) => Some('H'),
            (false, 0x24) => Some('j'),
            (true, 0x24) => Some('J'),
            (false, 0x25) => Some('k'),
            (true, 0x25) => Some('K'),
            (false, 0x26) => Some('l'),
            (true, 0x26) => Some('L'),

            (false, 0x2C) => Some('z'),
            (true, 0x2C) => Some('Z'),
            (false, 0x2D) => Some('x'),
            (true, 0x2D) => Some('X'),
            (false, 0x2E) => Some('c'),
            (true, 0x2E) => Some('C'),
            (false, 0x2F) => Some('v'),
            (true, 0x2F) => Some('V'),
            (false, 0x30) => Some('b'),
            (true, 0x30) => Some('B'),
            (false, 0x31) => Some('n'),
            (true, 0x31) => Some('N'),
            (false, 0x32) => Some('m'),
            (true, 0x32) => Some('M'),

            (_, 0x39) => Some(' '),

            (false, 0x0C) => Some('-'),
            (true, 0x0C) => Some('_'),
            (false, 0x0D) => Some('='),
            (true, 0x0D) => Some('+'),
            (false, 0x1A) => Some('['),
            (true, 0x1A) => Some('{'),
            (false, 0x1B) => Some(']'),
            (true, 0x1B) => Some('}'),
            (false, 0x27) => Some(';'),
            (true, 0x27) => Some(':'),
            (false, 0x28) => Some('\\'),
            (true, 0x28) => Some('|'),
            (false, 0x33) => Some(','),
            (true, 0x33) => Some('<'),
            (false, 0x34) => Some('.'),
            (true, 0x34) => Some('>'),
            (false, 0x35) => Some('/'),
            (true, 0x35) => Some('?'),
            (false, 0x0F) => Some('\t'),
            (true, 0x0F) => Some('\t'),
            (false, 0x2B) => Some('\\'),
            (true, 0x2B) => Some('|'),
            (false, 0x29) => Some('`'),
            (true, 0x29) => Some('~'),
            _ => None,
        },
    };

    DEADKEY.store(0, Ordering::Relaxed);
    result
}

fn scancode_to_screen(scancode: u8) -> Option<usize> {
    match scancode {
        0x3B..=0x44 => Some((scancode - 0x3B + 1) as usize),
        0x57 => Some(11),
        0x58 => Some(12),
        _ => None,
    }
}

pub fn process_scancode(scancode: u8) {
    if PREFIX.load(Ordering::Relaxed) == 0xE0 {
        PREFIX.store(0, Ordering::Relaxed);
        match scancode {
            0x1C => log::info!("Keypad Enter pressed"),
            0x38 => log::info!("Right Alt pressed"),
            0x48 => log::info!("Arrow Up pressed"),
            0x50 => log::info!("Arrow Down pressed"),
            0x4B => log::info!("Arrow Left pressed"),
            0x4D => log::info!("Arrow Right pressed"),
            0x47 => log::info!("Home pressed"),
            0x49 => log::info!("Page Up pressed"),
            0x4F => log::info!("End pressed"),
            0x51 => log::info!("Page Down pressed"),
            0x52 => log::info!("Insert pressed"),
            0x53 => log::info!("Delete pressed"),
            code => log::info!("Unknown E0-extended key: 0x{:02X}", code),
        }
        return;
    }

    if scancode == 0xE0 {
        PREFIX.store(0xE0, Ordering::Relaxed);
        return;
    }

    match scancode {
        0x2A | 0x36 => SHIFT.store(true, Ordering::Relaxed),
        0xAA | 0xB6 => SHIFT.store(false, Ordering::Relaxed),
        0x38 => ALTGR.store(true, Ordering::Relaxed),
        0xB8 => ALTGR.store(false, Ordering::Relaxed),
        0x01 => log::info!("Escape key pressed"),
        0x0E => log::info!("Backspace key pressed"),
        0x1C => log::info!("Enter key pressed"),
        0x39 => log::info!("Space key pressed"),
        0x3A => log::info!("Caps Lock key pressed"),
        0x3B..=0x44 | 0x57 | 0x58 => {
            if let Some(screen) = scancode_to_screen(scancode) {
                SELECTED_SCREEN.store(screen, Ordering::Relaxed);
                log::info!("Switched to screen {}", screen);
                if SYSTEM.lock().as_mut().is_some() {}
            }
        }
        code if code < 0x80 => {
            if let Some(c) = scancode_to_char(code) {
                serial_print!("{}", c);
            }
        }
        0x8F => {
            serial_print!("Tab key pressed");
        }
        0x80..=0xFF => {
            // Ignore key release codes or handle them as needed
        }
        _ => log::info!("Unhandled scancode: 0x{:02X}", scancode),
    }
}

use log::warn;
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub static KEYBOARD_BUFFER: Mutex<[u8; 256]> = Mutex::new([0; 256]);
pub static KEYBOARD_HEAD: AtomicUsize = AtomicUsize::new(0);

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut data_port = Port::<u8>::new(0x60);
    let scancode: u8 = unsafe { data_port.read() };

    let head = KEYBOARD_HEAD.fetch_add(1, Ordering::Relaxed) % 256;
    let mut buf = KEYBOARD_BUFFER.lock();
    buf[head] = scancode;

    end_of_interrupt(1);
}

pub fn pop_input() -> Option<u8> {
    let head = KEYBOARD_HEAD.load(Ordering::Acquire);
    if head == 0 {
        return None; // No input available
    }

    let mut buf = KEYBOARD_BUFFER.lock();
    let byte = buf[0];
    buf.copy_within(1..head, 0); // Shift buffer left
    KEYBOARD_HEAD.store(head - 1, Ordering::Release);

    Some(byte)
}

pub static MOUSE_PACKET_BUFFER: Mutex<[u8; 256]> = Mutex::new([0; 256]);
pub static MOUSE_HEAD: AtomicUsize = AtomicUsize::new(0);
pub static MOUSE_TAIL: AtomicUsize = AtomicUsize::new(0);

pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut data_port = Port::<u8>::new(0x60);
    let packet: u8 = unsafe { data_port.read() };

    let head = MOUSE_HEAD.load(Ordering::Relaxed);
    let next = (head + 1) % 256;

    let tail = MOUSE_TAIL.load(Ordering::Acquire);
    if next == tail {
        // Buffer full! Drop packet or handle overflow
        warn!("Mouse packet buffer overflow!");
        end_of_interrupt(12);
        return;
    }

    let mut buf = MOUSE_PACKET_BUFFER.lock();
    buf[head % 256] = packet;

    MOUSE_HEAD.store(next, Ordering::Release);
    end_of_interrupt(12);
}
