use crate::arch::x86_64::interrupts::end_of_interrupt;
use crate::drivers::input::InputBuffer;
use crate::serial_print;
use crate::task::runtime;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use log::{info, warn};
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub static SELECTED_SCREEN: AtomicUsize = AtomicUsize::new(1);

const KEYBOARD_BUFFER_LEN: usize = 256;

static SHIFT: AtomicBool = AtomicBool::new(false);
static ALTGR: AtomicBool = AtomicBool::new(false);
static CAPS_LOCK: AtomicBool = AtomicBool::new(false);
static DEADKEY: AtomicU8 = AtomicU8::new(DeadKey::None.to_u8());
static PREFIX: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeadKey {
    None,
    Acute,
    Grave,
    Circumflex,
    Diaeresis,
    Tilde,
}

impl DeadKey {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => DeadKey::Acute,
            2 => DeadKey::Grave,
            3 => DeadKey::Circumflex,
            4 => DeadKey::Diaeresis,
            5 => DeadKey::Tilde,
            _ => DeadKey::None,
        }
    }

    const fn to_u8(self) -> u8 {
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

#[derive(Clone, Copy)]
struct ModifierSnapshot {
    shift: bool,
    caps_lock: bool,
    altgr: bool,
    deadkey: DeadKey,
}

impl ModifierSnapshot {
    fn load() -> Self {
        Self {
            shift: SHIFT.load(Ordering::Relaxed),
            caps_lock: CAPS_LOCK.load(Ordering::Relaxed),
            altgr: ALTGR.load(Ordering::Relaxed),
            deadkey: DeadKey::from_u8(DEADKEY.load(Ordering::Relaxed)),
        }
    }
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
}

fn set_dead_key(dead: DeadKey) {
    DEADKEY.store(dead.to_u8(), Ordering::Relaxed);
}

fn clear_dead_key() {
    set_dead_key(DeadKey::None);
}

fn update_modifier(scancode: u8) -> bool {
    let released = scancode & 0x80 != 0;
    let code = scancode & 0x7F;

    match code {
        0x2A | 0x36 => {
            SHIFT.store(!released, Ordering::Relaxed);
            true
        }
        0x38 => {
            ALTGR.store(!released, Ordering::Relaxed);
            true
        }
        0x3A if !released => {
            CAPS_LOCK.fetch_xor(true, Ordering::Relaxed);
            true
        }
        _ => false,
    }
}

fn update_modifier_extended(scancode: u8) -> bool {
    let released = scancode & 0x80 != 0;

    match scancode {
        0x38 | 0xB8 => {
            ALTGR.store(!released, Ordering::Relaxed);
            true
        }
        _ => false,
    }
}

fn start_dead_key(scancode: u8, mods: &ModifierSnapshot) -> bool {
    match scancode {
        0x28 => {
            set_dead_key(DeadKey::Acute);
            true
        }
        0x29 => {
            set_dead_key(if mods.shift {
                DeadKey::Tilde
            } else {
                DeadKey::Grave
            });
            true
        }
        0x2B => {
            set_dead_key(DeadKey::Circumflex);
            true
        }
        0x1A => {
            set_dead_key(DeadKey::Diaeresis);
            true
        }
        _ => false,
    }
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

fn letter_from_scancode(scancode: u8, uppercase: bool) -> Option<char> {
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

    Some(if uppercase {
        letter.to_ascii_uppercase()
    } else {
        letter
    })
}

fn punctuation_from_scancode(scancode: u8, shift: bool) -> Option<char> {
    const PUNCT: &[(u8, char, char)] = &[
        (0x0C, '-', '_'),
        (0x0D, '=', '+'),
        (0x1A, '[', '{'),
        (0x1B, ']', '}'),
        (0x27, ';', ':'),
        (0x28, '\\', '|'),
        (0x2B, '\\', '|'),
        (0x33, ',', '<'),
        (0x34, '.', '>'),
        (0x35, '/', '?'),
        (0x29, '`', '~'),
    ];

    PUNCT
        .iter()
        .find(|(code, _, _)| *code == scancode)
        .map(|(_, normal, shifted)| if shift { *shifted } else { *normal })
}

fn scancode_to_char(scancode: u8, mods: &ModifierSnapshot) -> Option<char> {
    if mods.deadkey == DeadKey::None && start_dead_key(scancode, mods) {
        return None;
    }

    let shift = mods.shift || mods.altgr;
    let letter_shift = shift ^ mods.caps_lock;

    let raw = match scancode {
        0x02..=0x0B => {
            let idx = (scancode - 0x02) as usize;
            let digits = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];
            let shifted = ['!', '@', '#', '$', '%', '^', '&', '*', '(', ')'];
            if shift {
                shifted.get(idx).copied()
            } else {
                digits.get(idx).copied()
            }
        }
        0x10..=0x19 | 0x1E..=0x26 | 0x2C..=0x32 => letter_from_scancode(scancode, letter_shift),
        0x0F => Some('\t'),
        0x39 => Some(' '),
        _ => punctuation_from_scancode(scancode, shift),
    }?;

    if mods.deadkey == DeadKey::None {
        return Some(raw);
    }

    let composed = apply_dead_key(mods.deadkey, raw);
    clear_dead_key();
    composed.or(Some(raw))
}

fn decode_basic(scancode: u8, mods: &ModifierSnapshot) -> Option<KeyEvent> {
    let pressed = scancode & 0x80 == 0;
    let code = scancode & 0x7F;

    let key = match code {
        0x01 => KeyCode::Escape,
        0x0E => KeyCode::Backspace,
        0x0F => KeyCode::Tab,
        0x1C => KeyCode::Enter,
        0x39 => KeyCode::Space,
        0x3B..=0x44 => KeyCode::Function(code - 0x3A),
        0x57 => KeyCode::Function(11),
        0x58 => KeyCode::Function(12),
        0x46 => KeyCode::YieldNow,
        _ => {
            if !pressed {
                return None;
            }

            return scancode_to_char(code, mods).map(|c| KeyEvent {
                code: KeyCode::Printable(c),
                pressed,
            });
        }
    };

    Some(KeyEvent { code: key, pressed })
}

fn decode_extended(scancode: u8, _mods: &ModifierSnapshot) -> Option<KeyEvent> {
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

    Some(KeyEvent { code: key, pressed })
}

fn handle_function_key(idx: u8) {
    SELECTED_SCREEN.store(idx as usize, Ordering::Relaxed);
    let task_idx = idx.saturating_sub(1) as usize;
    if runtime::select_task(task_idx) {
        runtime::yield_now();
    }
}

fn handle_key_event(event: KeyEvent) {
    if !event.pressed {
        return;
    }

    match event.code {
        KeyCode::Printable(c) => serial_print!("{}", c),
        KeyCode::Tab => serial_print!("\t"),
        KeyCode::Space => serial_print!(" "),
        KeyCode::Backspace => info!("Backspace key pressed"),
        KeyCode::Enter => runtime::yield_now(),
        KeyCode::Escape => info!("Escape key pressed"),
        KeyCode::Function(idx) => handle_function_key(idx),
        KeyCode::YieldNow => runtime::yield_now(),
        KeyCode::ArrowUp => info!("Arrow Up pressed"),
        KeyCode::ArrowDown => info!("Arrow Down pressed"),
        KeyCode::ArrowLeft => info!("Arrow Left pressed"),
        KeyCode::ArrowRight => info!("Arrow Right pressed"),
        KeyCode::Home => info!("Home pressed"),
        KeyCode::End => info!("End pressed"),
        KeyCode::PageUp => info!("Page Up pressed"),
        KeyCode::PageDown => info!("Page Down pressed"),
        KeyCode::Insert => info!("Insert pressed"),
        KeyCode::Delete => info!("Delete pressed"),
        KeyCode::Unknown(code) => info!("Unhandled scancode: 0x{:02X}", code),
    }
}

pub fn process_scancode(scancode: u8) {
    if PREFIX.load(Ordering::Relaxed) == 0xE0 {
        PREFIX.store(0, Ordering::Relaxed);

        if update_modifier_extended(scancode) {
            return;
        }

        let mods = ModifierSnapshot::load();
        if let Some(event) = decode_extended(scancode, &mods) {
            handle_key_event(event);
        } else {
            info!("Unknown E0-extended key: 0x{:02X}", scancode);
        }

        return;
    }

    if scancode == 0xE0 {
        PREFIX.store(0xE0, Ordering::Relaxed);
        return;
    }

    if update_modifier(scancode) {
        return;
    }

    let mods = ModifierSnapshot::load();
    if let Some(event) = decode_basic(scancode, &mods) {
        handle_key_event(event);
    }
}

pub static KEYBOARD_BUFFER: InputBuffer<u8, KEYBOARD_BUFFER_LEN> = InputBuffer::new(0);

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut data_port = Port::<u8>::new(0x60);
    let scancode: u8 = unsafe { data_port.read() };

    if KEYBOARD_BUFFER.push(scancode).is_err() {
        warn!(
            "Keyboard buffer overflow, dropping scancode 0x{:02X}",
            scancode
        );
    }

    // Process the scancode immediately since there is no dedicated
    // input thread yet. This keeps task switching via keyboard working.
    process_scancode(scancode);

    end_of_interrupt(1);
}

pub fn pop_input() -> Option<u8> {
    KEYBOARD_BUFFER.pop()
}
