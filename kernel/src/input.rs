use crate::interrupts::end_of_interrupt;
use crate::serial_print;
use core::sync::atomic::{AtomicBool, Ordering};

/// Modifier state
static SHIFT: AtomicBool = AtomicBool::new(false);
static ALTGR: AtomicBool = AtomicBool::new(false);

/// Basic US International Set 1 scancode map
pub fn scancode_to_char(scancode: u8) -> Option<char> {
    let shift = SHIFT.load(Ordering::Relaxed);
    let altgr = ALTGR.load(Ordering::Relaxed);

    let ch = match (altgr, shift, scancode) {
        // AltGr International mappings
        (true, _, 0x10) => Some('á'), // Q => á
        (true, _, 0x11) => Some('ß'), // W => ß
        (true, _, 0x12) => Some('€'), // E => €
        (true, _, 0x13) => Some('ð'), // R => ð
        (true, _, 0x14) => Some('þ'), // T => þ
        (true, _, 0x15) => Some('ý'), // Y => ý
        (true, _, 0x16) => Some('ú'), // U => ú
        (true, _, 0x17) => Some('í'), // I => í
        (true, _, 0x18) => Some('ó'), // O => ó
        (true, _, 0x19) => Some('ö'), // P => ö
        (true, _, 0x1A) => Some('«'), // [ => «
        (true, _, 0x1B) => Some('»'), // ] => »
        (true, _, 0x1E) => Some('á'), // A => á
        (true, _, 0x1F) => Some('ß'), // S => ß
        (true, _, 0x20) => Some('ð'), // D => ð
        (true, _, 0x21) => Some('æ'), // F => æ
        (true, _, 0x22) => Some('œ'), // G => œ
        (true, _, 0x23) => Some('ø'), // H => ø
        (true, _, 0x24) => Some('¶'), // J => ¶
        (true, _, 0x25) => Some('ß'), // K => ß
        (true, _, 0x26) => Some('ł'), // L => ł
        (true, _, 0x2C) => Some('ç'), // Z => ç
        (true, _, 0x2D) => Some('ñ'), // X => ñ
        (true, _, 0x2E) => Some('µ'), // C => µ
        (true, _, 0x2F) => Some('ç'), // V => ç
        (true, _, 0x30) => Some('þ'), // B => þ
        (true, _, 0x31) => Some('ñ'), // N => ñ
        (true, _, 0x32) => Some('µ'), // M => µ

        // Normal mappings
        (false, false, 0x02) => Some('1'),
        (false, true, 0x02) => Some('!'),
        (false, false, 0x03) => Some('2'),
        (false, true, 0x03) => Some('@'),
        (false, false, 0x04) => Some('3'),
        (false, true, 0x04) => Some('#'),
        (false, false, 0x05) => Some('4'),
        (false, true, 0x05) => Some('$'),
        (false, false, 0x06) => Some('5'),
        (false, true, 0x06) => Some('%'),
        (false, false, 0x07) => Some('6'),
        (false, true, 0x07) => Some('^'),
        (false, false, 0x08) => Some('7'),
        (false, true, 0x08) => Some('&'),
        (false, false, 0x09) => Some('8'),
        (false, true, 0x09) => Some('*'),
        (false, false, 0x0A) => Some('9'),
        (false, true, 0x0A) => Some('('),
        (false, false, 0x0B) => Some('0'),
        (false, true, 0x0B) => Some(')'),
        (false, false, 0x10) => Some('q'),
        (false, true, 0x10) => Some('Q'),
        (false, false, 0x11) => Some('w'),
        (false, true, 0x11) => Some('W'),
        (false, false, 0x12) => Some('e'),
        (false, true, 0x12) => Some('E'),
        (false, false, 0x13) => Some('r'),
        (false, true, 0x13) => Some('R'),
        (false, false, 0x14) => Some('t'),
        (false, true, 0x14) => Some('T'),
        (false, false, 0x15) => Some('y'),
        (false, true, 0x15) => Some('Y'),
        (false, false, 0x16) => Some('u'),
        (false, true, 0x16) => Some('U'),
        (false, false, 0x17) => Some('i'),
        (false, true, 0x17) => Some('I'),
        (false, false, 0x18) => Some('o'),
        (false, true, 0x18) => Some('O'),
        (false, false, 0x19) => Some('p'),
        (false, true, 0x19) => Some('P'),
        (false, false, 0x1E) => Some('a'),
        (false, true, 0x1E) => Some('A'),
        (false, false, 0x1F) => Some('s'),
        (false, true, 0x1F) => Some('S'),
        (false, false, 0x20) => Some('d'),
        (false, true, 0x20) => Some('D'),
        (false, false, 0x21) => Some('f'),
        (false, true, 0x21) => Some('F'),
        (false, false, 0x22) => Some('g'),
        (false, true, 0x22) => Some('G'),
        (false, false, 0x23) => Some('h'),
        (false, true, 0x23) => Some('H'),
        (false, false, 0x24) => Some('j'),
        (false, true, 0x24) => Some('J'),
        (false, false, 0x25) => Some('k'),
        (false, true, 0x25) => Some('K'),
        (false, false, 0x26) => Some('l'),
        (false, true, 0x26) => Some('L'),
        (false, false, 0x2C) => Some('z'),
        (false, true, 0x2C) => Some('Z'),
        (false, false, 0x2D) => Some('x'),
        (false, true, 0x2D) => Some('X'),
        (false, false, 0x2E) => Some('c'),
        (false, true, 0x2E) => Some('C'),
        (false, false, 0x2F) => Some('v'),
        (false, true, 0x2F) => Some('V'),
        (false, false, 0x30) => Some('b'),
        (false, true, 0x30) => Some('B'),
        (false, false, 0x31) => Some('n'),
        (false, true, 0x31) => Some('N'),
        (false, false, 0x32) => Some('m'),
        (false, true, 0x32) => Some('M'),
        (false, false, 0x39) => Some(' '),
        _ => None,
    };

    ch
}

/// Process a full stream of scancodes (presses and releases)
pub fn process_scancode(scancode: u8) {
    match scancode {
        0x2A | 0x36 => SHIFT.store(true, Ordering::Relaxed), // LShift / RShift press
        0xAA | 0xB6 => SHIFT.store(false, Ordering::Relaxed), // LShift / RShift release
        0x38 => ALTGR.store(true, Ordering::Relaxed),        // AltGr (Right Alt) press
        0xB8 => ALTGR.store(false, Ordering::Relaxed),       // AltGr release
        code if code < 0x80 => {
            if let Some(c) = scancode_to_char(code) {
                serial_print!("{}", c);
            }
        }
        _ => {} // ignore key releases
    }
}
use core::sync::atomic::AtomicUsize;
use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrame;

pub static KEYBOARD_BUFFER: Mutex<[u8; 256]> = Mutex::new([0; 256]);
pub static KEYBOARD_HEAD: AtomicUsize = AtomicUsize::new(0);

pub struct AtomicPoint {
    pub x: AtomicUsize,
    pub y: AtomicUsize,
}

pub static MOUSE_POS: AtomicPoint = AtomicPoint {
    x: AtomicUsize::new(0),
    y: AtomicUsize::new(0),
};

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut data_port = Port::<u8>::new(0x60);
    let scancode: u8 = unsafe { data_port.read() };

    let head = KEYBOARD_HEAD.fetch_add(1, Ordering::Relaxed) % 256;
    let mut buf = KEYBOARD_BUFFER.lock();
    buf[head] = scancode;

    end_of_interrupt(1);
}

pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let x = MOUSE_POS.x.load(Ordering::Relaxed);
    let y = MOUSE_POS.y.load(Ordering::Relaxed);

    MOUSE_POS.x.store(x + 1, Ordering::Relaxed);
    MOUSE_POS.y.store(y + 1, Ordering::Relaxed);

    end_of_interrupt(12); // IRQ12, not 44
}
