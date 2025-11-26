//! Symbol table helpers. Keep codes short and human-readable for dumps.

use core::fmt;
use serde::{Deserialize, Serialize};

/// Compact human-readable symbol (2–3 bytes packed into a `u32`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct Symbol(pub u32);

impl Symbol {
    pub const fn new(raw: u32) -> Self {
        Symbol(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Combine two ASCII chars into a human-readable code.
pub const fn cc(a: char, b: char) -> Symbol {
    Symbol(((a as u32) << 16) | ((b as u32) << 8))
}

/// Combine three ASCII bytes into a human-readable code.
pub const fn canon(a: u8, b: u8, c: u8) -> Symbol {
    Symbol(((a as u32) << 16) | ((b as u32) << 8) | (c as u32))
}

pub const fn from_char(c: char) -> Symbol {
    Symbol(c as u32)
}

pub const fn from_u16(raw: u16) -> Symbol {
    Symbol(raw as u32)
}

// Common symbols used by early drivers and journal dumps.
pub const JOURNAL: Symbol = cc('J', 'N');
pub const KEYBOARD: Symbol = cc('K', 'B');
pub const KEY_PRESSED: Symbol = cc('K', 'P');
pub const MOUSE: Symbol = cc('M', 'S');
pub const MOUSE_MOVED: Symbol = cc('M', 'V');
pub const AT: Symbol = cc('@', ' ');
pub const INIT: Symbol = cc('I', 'N');
pub const FAIL: Symbol = cc('F', 'L');
pub const DRIVER: Symbol = cc('D', 'R');
pub const THING_CREATED: Symbol = cc('T', 'C');
pub const EDGE_ADDED: Symbol = cc('E', 'D');

// Common data field keys.
pub const ID: Symbol = canon(b'I', b'D', b' ');
pub const KIND: Symbol = canon(b'K', b'N', b'D');
pub const FIELDS: Symbol = canon(b'F', b'L', b'D');
pub const REVISION: Symbol = canon(b'R', b'V', b'N');
pub const SRC: Symbol = canon(b'S', b'R', b'C');
pub const DST: Symbol = canon(b'D', b'S', b'T');
pub const PREDICATE: Symbol = canon(b'P', b'R', b'D');
pub const NAME: Symbol = canon(b'N', b'A', b'M');
pub const STATUS: Symbol = canon(b'S', b'T', b'A');
pub const SCANCODE: Symbol = canon(b'S', b'C', b'N');
pub const KEY: Symbol = canon(b'K', b'E', b'Y');
pub const DX: Symbol = canon(b'D', b'X', b' ');
pub const DY: Symbol = canon(b'D', b'Y', b' ');
pub const BUTTONS: Symbol = canon(b'B', b'T', b'N');
pub const WRITE: Symbol = canon(b'W', b'R', b'T');
pub const TARGET: Symbol = canon(b'T', b'G', b'T');
pub const TEXT: Symbol = canon(b'T', b'X', b'T');
pub const STDOUT: Symbol = canon(b'S', b'T', b'D');
pub const ADDR: Symbol = canon(b'A', b'D', b'R');
pub const WIDTH: Symbol = canon(b'W', b'D', b'T');
pub const HEIGHT: Symbol = canon(b'H', b'G', b'T');
pub const PITCH: Symbol = canon(b'P', b'T', b'H');
pub const BPP: Symbol = canon(b'B', b'P', b'P');
pub const COMPOSITOR: Symbol = canon(b'C', b'M', b'P');
pub const WINDOW: Symbol = canon(b'W', b'I', b'N');
pub const PIXMAP: Symbol = canon(b'P', b'X', b'M');
pub const STREAMS: Symbol = canon(b'S', b'T', b'M');
pub const COMPOSED_BY: Symbol = canon(b'C', b'M', b'B');
pub const WINDOW_CREATED: Symbol = canon(b'W', b'C', b'R');
pub const WINDOW_BUFFER_UPDATED: Symbol = canon(b'W', b'B', b'U');
pub const FRAME_READY: Symbol = canon(b'F', b'R', b'M');

pub const DRIVER_INPUT: Symbol = canon(b'I', b'N', b'P');
pub const DRIVER_DISPLAY: Symbol = canon(b'D', b'S', b'P');
pub const DRIVER_STORAGE: Symbol = canon(b'S', b'T', b'R');
pub const DRIVER_TIMER: Symbol = canon(b'T', b'M', b'R');
pub const DRIVER_OTHER: Symbol = canon(b'O', b'T', b'H');
pub const EMITS: Symbol = canon(b'E', b'M', b'T');

pub const INPUT_DEVICE_MOUSE: Symbol = canon(b'I', b'D', b'M');
pub const INPUT_EVENT: Symbol = canon(b'I', b'E', b'V');
pub const MOVE: Symbol = canon(b'M', b'O', b'V');
pub const DEVICE_ID: Symbol = canon(b'D', b'I', b'D');
pub const TS: Symbol = canon(b'T', b'S', b' ');
pub const DOWN: Symbol = canon(b'D', b'W', b'N');
pub const BUTTON: Symbol = canon(b'B', b'T', b'#');

/// Return the symbolic name for a code if known.
pub fn sym_name(code: Symbol) -> &'static str {
    // Linear scan keeps it tiny; expand or sort if this grows.
    const TBL: &[(Symbol, &str)] = &[
        (JOURNAL, "journal"),
        (KEYBOARD, "keyboard"),
        (KEY_PRESSED, "key_pressed"),
        (MOUSE, "mouse"),
        (MOUSE_MOVED, "mouse_moved"),
        (INPUT_DEVICE_MOUSE, "input.device.mouse"),
        (INPUT_EVENT, "input.event"),
        (MOVE, "move"),
        (DEVICE_ID, "device_id"),
        (TS, "ts"),
        (DOWN, "down"),
        (BUTTON, "button"),
        (AT, "at"),
        (INIT, "init"),
        (FAIL, "fail"),
        (DRIVER, "driver"),
        (THING_CREATED, "thing_created"),
        (EDGE_ADDED, "edge_added"),
        (ID, "id"),
        (KIND, "kind"),
        (FIELDS, "fields"),
        (REVISION, "revision"),
        (SRC, "src"),
        (DST, "dst"),
        (PREDICATE, "predicate"),
        (NAME, "name"),
        (STATUS, "status"),
        (SCANCODE, "scancode"),
        (KEY, "key"),
        (DX, "dx"),
        (DY, "dy"),
        (BUTTONS, "buttons"),
        (WRITE, "write"),
        (TARGET, "target"),
        (TEXT, "text"),
        (STDOUT, "stdout"),
        (ADDR, "addr"),
        (WIDTH, "width"),
        (HEIGHT, "height"),
        (PITCH, "pitch"),
        (BPP, "bpp"),
        (COMPOSITOR, "compositor"),
        (WINDOW, "window"),
        (PIXMAP, "pixmap"),
        (STREAMS, "streams"),
        (COMPOSED_BY, "composed_by"),
        (WINDOW_CREATED, "window_created"),
        (WINDOW_BUFFER_UPDATED, "window_buffer_updated"),
        (FRAME_READY, "frame_ready"),
        (DRIVER_INPUT, "driver_input"),
        (DRIVER_DISPLAY, "driver_display"),
        (DRIVER_STORAGE, "driver_storage"),
        (DRIVER_TIMER, "driver_timer"),
        (DRIVER_OTHER, "driver_other"),
    ];
    let mut i = 0;
    while i < TBL.len() {
        if TBL[i].0 == code {
            return TBL[i].1;
        }
        i += 1;
    }
    ""
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = sym_name(*self);
        if name.is_empty() {
            write!(f, "0x{:06x}", self.raw())
        } else {
            f.write_str(name)
        }
    }
}
