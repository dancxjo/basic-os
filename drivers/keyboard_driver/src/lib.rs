#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;

use userland::prelude::*;
use userland::{canon, sys, Symbol};
use uuid::Uuid;

pub struct KeyboardDriver {
    device_id: Uuid,
    device_handle: Option<u64>,
    irq_handle: Option<u64>,
    watch: WatchId,
    prefix: Option<u8>,
    shift: bool,
    caps_lock: bool,
    num_lock: bool,
}

impl App for KeyboardDriver {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let device_id = device_id();
        let device_handle = sys::dev_open(sys::DEVICE_KIND_KEYBOARD, 0);
        let irq_handle = sys::irq_bind(sys::IrqBindRequest {
            device: device_id,
            irq_line: KEYBOARD_IRQ_LINE,
        });

        let watch = ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });

        KeyboardDriver {
            device_id,
            device_handle,
            irq_handle,
            watch,
            prefix: None,
            shift: false,
            caps_lock: false,
            num_lock: false,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        if let Some(handle) = self.irq_handle {
            let mut buf = [0u8; 16];
            if let Some(dev) = self.device_handle {
                loop {
                    let n = sys::dev_read(dev, &mut buf);
                    if n == 0 {
                        break;
                    }
                    for i in 0..n {
                        self.handle_scancode(buf[i]);
                    }
                }
            }
            let _ = sys::irq_ack(handle);
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { watch, thing } = ev {
            if watch == self.watch && thing.kind == canon::KEY_EVENT {
                let _scancode = thing
                    .fields
                    .get(&canon::SCANCODE)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let _down = thing
                    .fields
                    .get(&canon::DOWN)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // println!("key event: scancode=0x{:02x} down={}", scancode, down);
            }
        }
    }
}

impl KeyboardDriver {
    fn handle_scancode(&mut self, scancode: u8) {
        if let Some(0xE0) = self.prefix {
            self.prefix = None;
            self.emit_event(scancode, true);
            return;
        }

        if scancode == 0xE0 {
            self.prefix = Some(0xE0);
            return;
        }

        self.emit_event(scancode, false);
    }

    fn emit_event(&mut self, scancode: u8, extended: bool) {
        let down = scancode & 0x80 == 0;
        let code = scancode & 0x7F;
        let key_code = self.decode_key(code, extended);
        self.update_state(key_code, down);

        let (key_symbol, key_text) = self.describe_key(key_code, down);

        let mut fields = BTreeMap::new();
        fields.insert(canon::DEVICE_ID, Value::Uuid(self.device_id));
        fields.insert(canon::SCANCODE, Value::U64(code as u64));
        fields.insert(canon::DOWN, Value::Bool(down));
        if let Some(sym) = key_symbol {
            fields.insert(canon::KEY, Value::Symbol(sym));
        }
        if !key_text.is_empty() {
            fields.insert(canon::TEXT, Value::Text(key_text.clone()));
        }

        let _ = fiat(None, canon::KEY_EVENT, fields.clone());

        if down {
            let _ = fiat(None, canon::KEY_PRESSED, fields);
        }
    }

    fn decode_key(&self, code: u8, extended: bool) -> KeyCode {
        if extended {
            return match code {
                0x1C => KeyCode::KeypadEnter,
                0x1D => KeyCode::CtrlRight,
                0x35 => KeyCode::KeypadDivide,
                0x38 => KeyCode::AltRight,
                0x47 => KeyCode::Home,
                0x48 => KeyCode::ArrowUp,
                0x49 => KeyCode::PageUp,
                0x4B => KeyCode::ArrowLeft,
                0x4D => KeyCode::ArrowRight,
                0x4F => KeyCode::End,
                0x50 => KeyCode::ArrowDown,
                0x51 => KeyCode::PageDown,
                0x52 => KeyCode::Insert,
                0x53 => KeyCode::Delete,
                0x5B => KeyCode::GuiLeft,
                0x5C => KeyCode::GuiRight,
                0x5D => KeyCode::ContextMenu,
                _ => KeyCode::Unknown {
                    code,
                    extended: true,
                },
            };
        }

        match code {
            0x01 => KeyCode::Escape,
            0x02 => KeyCode::Character {
                base: '1',
                shifted: Some('!'),
            },
            0x03 => KeyCode::Character {
                base: '2',
                shifted: Some('@'),
            },
            0x04 => KeyCode::Character {
                base: '3',
                shifted: Some('#'),
            },
            0x05 => KeyCode::Character {
                base: '4',
                shifted: Some('$'),
            },
            0x06 => KeyCode::Character {
                base: '5',
                shifted: Some('%'),
            },
            0x07 => KeyCode::Character {
                base: '6',
                shifted: Some('^'),
            },
            0x08 => KeyCode::Character {
                base: '7',
                shifted: Some('&'),
            },
            0x09 => KeyCode::Character {
                base: '8',
                shifted: Some('*'),
            },
            0x0A => KeyCode::Character {
                base: '9',
                shifted: Some('('),
            },
            0x0B => KeyCode::Character {
                base: '0',
                shifted: Some(')'),
            },
            0x0C => KeyCode::Character {
                base: '-',
                shifted: Some('_'),
            },
            0x0D => KeyCode::Character {
                base: '=',
                shifted: Some('+'),
            },
            0x0E => KeyCode::Backspace,
            0x0F => KeyCode::Tab,
            0x10 => KeyCode::Character {
                base: 'q',
                shifted: None,
            },
            0x11 => KeyCode::Character {
                base: 'w',
                shifted: None,
            },
            0x12 => KeyCode::Character {
                base: 'e',
                shifted: None,
            },
            0x13 => KeyCode::Character {
                base: 'r',
                shifted: None,
            },
            0x14 => KeyCode::Character {
                base: 't',
                shifted: None,
            },
            0x15 => KeyCode::Character {
                base: 'y',
                shifted: None,
            },
            0x16 => KeyCode::Character {
                base: 'u',
                shifted: None,
            },
            0x17 => KeyCode::Character {
                base: 'i',
                shifted: None,
            },
            0x18 => KeyCode::Character {
                base: 'o',
                shifted: None,
            },
            0x19 => KeyCode::Character {
                base: 'p',
                shifted: None,
            },
            0x1A => KeyCode::Character {
                base: '[',
                shifted: Some('{'),
            },
            0x1B => KeyCode::Character {
                base: ']',
                shifted: Some('}'),
            },
            0x1C => KeyCode::Enter,
            0x1D => KeyCode::CtrlLeft,
            0x1E => KeyCode::Character {
                base: 'a',
                shifted: None,
            },
            0x1F => KeyCode::Character {
                base: 's',
                shifted: None,
            },
            0x20 => KeyCode::Character {
                base: 'd',
                shifted: None,
            },
            0x21 => KeyCode::Character {
                base: 'f',
                shifted: None,
            },
            0x22 => KeyCode::Character {
                base: 'g',
                shifted: None,
            },
            0x23 => KeyCode::Character {
                base: 'h',
                shifted: None,
            },
            0x24 => KeyCode::Character {
                base: 'j',
                shifted: None,
            },
            0x25 => KeyCode::Character {
                base: 'k',
                shifted: None,
            },
            0x26 => KeyCode::Character {
                base: 'l',
                shifted: None,
            },
            0x27 => KeyCode::Character {
                base: ';',
                shifted: Some(':'),
            },
            0x28 => KeyCode::Character {
                base: '\'',
                shifted: Some('"'),
            },
            0x29 => KeyCode::Character {
                base: '`',
                shifted: Some('~'),
            },
            0x2A => KeyCode::ShiftLeft,
            0x2B => KeyCode::Character {
                base: '\\',
                shifted: Some('|'),
            },
            0x2C => KeyCode::Character {
                base: 'z',
                shifted: None,
            },
            0x2D => KeyCode::Character {
                base: 'x',
                shifted: None,
            },
            0x2E => KeyCode::Character {
                base: 'c',
                shifted: None,
            },
            0x2F => KeyCode::Character {
                base: 'v',
                shifted: None,
            },
            0x30 => KeyCode::Character {
                base: 'b',
                shifted: None,
            },
            0x31 => KeyCode::Character {
                base: 'n',
                shifted: None,
            },
            0x32 => KeyCode::Character {
                base: 'm',
                shifted: None,
            },
            0x33 => KeyCode::Character {
                base: ',',
                shifted: Some('<'),
            },
            0x34 => KeyCode::Character {
                base: '.',
                shifted: Some('>'),
            },
            0x35 => KeyCode::Character {
                base: '/',
                shifted: Some('?'),
            },
            0x36 => KeyCode::ShiftRight,
            0x37 => KeyCode::KeypadMultiply,
            0x38 => KeyCode::AltLeft,
            0x39 => KeyCode::Character {
                base: ' ',
                shifted: None,
            },
            0x3A => KeyCode::CapsLock,
            0x3B..=0x44 => KeyCode::Function(code as u8 - 0x3A),
            0x45 => KeyCode::NumLock,
            0x46 => KeyCode::ScrollLock,
            0x47 => KeyCode::Keypad(7),
            0x48 => KeyCode::Keypad(8),
            0x49 => KeyCode::Keypad(9),
            0x4A => KeyCode::KeypadSubtract,
            0x4B => KeyCode::Keypad(4),
            0x4C => KeyCode::Keypad(5),
            0x4D => KeyCode::Keypad(6),
            0x4E => KeyCode::KeypadAdd,
            0x4F => KeyCode::Keypad(1),
            0x50 => KeyCode::Keypad(2),
            0x51 => KeyCode::Keypad(3),
            0x52 => KeyCode::Keypad(0),
            0x53 => KeyCode::KeypadDecimal,
            0x57 => KeyCode::Function(11),
            0x58 => KeyCode::Function(12),
            _ => KeyCode::Unknown {
                code,
                extended: false,
            },
        }
    }

    fn update_state(&mut self, key_code: KeyCode, down: bool) {
        match key_code {
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.shift = down;
            }
            KeyCode::CapsLock if down => {
                self.caps_lock = !self.caps_lock;
            }
            KeyCode::NumLock if down => {
                self.num_lock = !self.num_lock;
            }
            _ => {}
        }
    }

    fn describe_key(&self, key_code: KeyCode, down: bool) -> (Option<Symbol>, String) {
        if let Some(ch) = self.key_char(key_code) {
            let symbol = canon::from_char(ch);
            let text = if ch == ' ' {
                "Space".to_string()
            } else {
                ch.to_string()
            };
            return (Some(symbol), text);
        }

        let symbol = key_symbol(key_code);
        let text = match key_code {
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Escape => "Escape".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::CtrlLeft => "Left Ctrl".to_string(),
            KeyCode::CtrlRight => "Right Ctrl".to_string(),
            KeyCode::ShiftLeft => "Left Shift".to_string(),
            KeyCode::ShiftRight => "Right Shift".to_string(),
            KeyCode::AltLeft => "Left Alt".to_string(),
            KeyCode::AltRight => "Right Alt".to_string(),
            KeyCode::CapsLock => format!("Caps Lock ({})", on_off(self.caps_lock)),
            KeyCode::NumLock => format!("Num Lock ({})", on_off(self.num_lock)),
            KeyCode::ScrollLock => "Scroll Lock".to_string(),
            KeyCode::Function(idx) => format!("F{}", idx),
            KeyCode::ArrowUp => "Arrow Up".to_string(),
            KeyCode::ArrowDown => "Arrow Down".to_string(),
            KeyCode::ArrowLeft => "Arrow Left".to_string(),
            KeyCode::ArrowRight => "Arrow Right".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "Page Up".to_string(),
            KeyCode::PageDown => "Page Down".to_string(),
            KeyCode::Insert => "Insert".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::GuiLeft => "Left Meta".to_string(),
            KeyCode::GuiRight => "Right Meta".to_string(),
            KeyCode::ContextMenu => "Menu".to_string(),
            KeyCode::Keypad(n) => self.describe_keypad_digit(n),
            KeyCode::KeypadDecimal => {
                if self.num_lock {
                    "Numpad .".to_string()
                } else {
                    "Delete".to_string()
                }
            }
            KeyCode::KeypadAdd => "Numpad +".to_string(),
            KeyCode::KeypadSubtract => "Numpad -".to_string(),
            KeyCode::KeypadMultiply => "Numpad *".to_string(),
            KeyCode::KeypadDivide => "Numpad /".to_string(),
            KeyCode::KeypadEnter => "Numpad Enter".to_string(),
            KeyCode::Unknown { code, extended } => format!(
                "Unknown 0x{:02X}{}{}",
                code,
                if extended { " (E0)" } else { "" },
                if down { "" } else { " released" }
            ),
            KeyCode::Character { .. } => String::new(),
        };

        (symbol, text)
    }

    fn describe_keypad_digit(&self, digit: u8) -> String {
        if self.num_lock || digit == 5 {
            return format!("Numpad {}", digit);
        }

        match digit {
            7 => "Home".to_string(),
            8 => "Arrow Up".to_string(),
            9 => "Page Up".to_string(),
            4 => "Arrow Left".to_string(),
            6 => "Arrow Right".to_string(),
            1 => "End".to_string(),
            2 => "Arrow Down".to_string(),
            3 => "Page Down".to_string(),
            0 => "Insert".to_string(),
            _ => format!("Numpad {}", digit),
        }
    }

    fn key_char(&self, key_code: KeyCode) -> Option<char> {
        match key_code {
            KeyCode::Character { base, shifted } => {
                if base.is_ascii_alphabetic() {
                    if self.shift ^ self.caps_lock {
                        Some(base.to_ascii_uppercase())
                    } else {
                        Some(base)
                    }
                } else if self.shift {
                    Some(shifted.unwrap_or(base))
                } else {
                    Some(base)
                }
            }
            KeyCode::Keypad(d) if self.num_lock => Some((b'0' + d) as char),
            KeyCode::KeypadDecimal if self.num_lock => Some('.'),
            KeyCode::KeypadAdd => Some('+'),
            KeyCode::KeypadSubtract => Some('-'),
            KeyCode::KeypadMultiply => Some('*'),
            KeyCode::KeypadDivide => Some('/'),
            KeyCode::KeypadEnter => None,
            _ => None,
        }
    }
}

const KEYBOARD_DEVICE_NAME: &str = "ps2-keyboard0";
const KEYBOARD_IRQ_LINE: u8 = 1;

fn device_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, KEYBOARD_DEVICE_NAME.as_bytes())
}

app_main!(KeyboardDriver);

#[derive(Clone, Copy)]
enum KeyCode {
    Character { base: char, shifted: Option<char> },
    Enter,
    Escape,
    Backspace,
    Tab,
    CtrlLeft,
    CtrlRight,
    ShiftLeft,
    ShiftRight,
    AltLeft,
    AltRight,
    CapsLock,
    NumLock,
    ScrollLock,
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
    GuiLeft,
    GuiRight,
    ContextMenu,
    Keypad(u8),
    KeypadDecimal,
    KeypadAdd,
    KeypadSubtract,
    KeypadMultiply,
    KeypadDivide,
    KeypadEnter,
    Unknown { code: u8, extended: bool },
}

fn key_symbol(code: KeyCode) -> Option<Symbol> {
    Some(match code {
        KeyCode::Character { .. } => return None,
        KeyCode::Enter => canon::cc('E', 'N'),
        KeyCode::Escape => canon::cc('E', 'S'),
        KeyCode::Backspace => canon::cc('B', 'S'),
        KeyCode::Tab => canon::cc('T', 'B'),
        KeyCode::CtrlLeft | KeyCode::CtrlRight => canon::cc('C', 'T'),
        KeyCode::ShiftLeft | KeyCode::ShiftRight => canon::cc('S', 'F'),
        KeyCode::AltLeft | KeyCode::AltRight => canon::cc('A', 'L'),
        KeyCode::CapsLock => canon::cc('C', 'L'),
        KeyCode::NumLock => canon::cc('N', 'L'),
        KeyCode::ScrollLock => canon::cc('S', 'L'),
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
        KeyCode::GuiLeft | KeyCode::GuiRight => canon::cc('G', 'U'),
        KeyCode::ContextMenu => canon::cc('M', 'N'),
        KeyCode::Keypad(_) => canon::cc('K', 'P'),
        KeyCode::KeypadDecimal => canon::cc('K', '.'),
        KeyCode::KeypadAdd => canon::cc('K', '+'),
        KeyCode::KeypadSubtract => canon::cc('K', '-'),
        KeyCode::KeypadMultiply => canon::cc('K', '*'),
        KeyCode::KeypadDivide => canon::cc('K', '/'),
        KeyCode::KeypadEnter => canon::cc('K', 'E'),
        KeyCode::Unknown { .. } => return None,
    })
}

fn on_off(enabled: bool) -> &'static str {
    if enabled {
        "on"
    } else {
        "off"
    }
}
