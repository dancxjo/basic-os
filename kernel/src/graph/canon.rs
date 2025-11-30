//! Symbol table helpers. Keep codes short and human-readable for dumps.

use alloc::string::String;
use alloc::vec::Vec;
pub use thing_abi::{Symbol, canon, cc, from_char, from_u16};

// Common symbols used by early drivers and journal dumps.
pub const JOURNAL: Symbol = cc('J', 'N');
pub const KEYBOARD: Symbol = cc('K', 'B');
pub const KEY_PRESSED: Symbol = cc('K', 'P');
pub const KEY_EVENT: Symbol = canon(b'K', b'E', b'V');
pub const MOUSE: Symbol = cc('M', 'S');
pub const MOUSE_MOVED: Symbol = cc('M', 'V');
pub const MOUSE_MOVE: Symbol = canon(b'M', b'M', b'V');
pub const MOUSE_BUTTON: Symbol = canon(b'M', b'B', b'T');
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
pub const BUNDLE: Symbol = canon(b'B', b'N', b'D');
pub const OWNER: Symbol = canon(b'O', b'W', b'N');
pub const OWNS: Symbol = canon(b'O', b'W', b'S');
pub const CAN_READ: Symbol = canon(b'C', b'R', b'D');
pub const CAN_WRITE: Symbol = canon(b'C', b'W', b'R');
pub const CAN_LINK: Symbol = canon(b'C', b'L', b'K');
pub const CAN_HANDLE_IRQ: Symbol = canon(b'C', b'I', b'Q');
pub const CAN_DMA: Symbol = canon(b'C', b'D', b'M');
pub const CAN_MMIO: Symbol = canon(b'C', b'M', b'M');
pub const CAN_PORT_IO: Symbol = canon(b'C', b'P', b'O');
pub const ADDR: Symbol = canon(b'A', b'D', b'R');
pub const X: Symbol = canon(b'X', b' ', b' ');
pub const Y: Symbol = canon(b'Y', b' ', b' ');
pub const Z: Symbol = canon(b'Z', b'I', b'N');
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
pub const SURFACE: Symbol = canon(b'S', b'F', b'C');
pub const CURSOR: Symbol = canon(b'C', b'R', b'S');
pub const SHARED_BUFFER: Symbol = canon(b'S', b'B', b'F');
pub const QUEUE_STATE: Symbol = canon(b'Q', b'S', b'T');
pub const BUFFER_KIND: Symbol = canon(b'B', b'K', b'D');
pub const BUFFER_USAGE: Symbol = canon(b'B', b'U', b'G');
pub const RING: Symbol = canon(b'R', b'I', b'N');
pub const LINEAR: Symbol = canon(b'L', b'I', b'N');
pub const PIPE_USAGE: Symbol = canon(b'P', b'I', b'P');
pub const SURFACE_USAGE: Symbol = canon(b'S', b'R', b'F');
pub const RX_RING_USAGE: Symbol = canon(b'R', b'X', b'R');
pub const HEAD: Symbol = canon(b'H', b'E', b'A');
pub const TAIL: Symbol = canon(b'T', b'A', b'I');
pub const HAS_DATA: Symbol = canon(b'H', b'D', b'T');
pub const CAPACITY: Symbol = canon(b'C', b'A', b'P');

pub const DRIVER_INPUT: Symbol = canon(b'I', b'N', b'P');
pub const DRIVER_DISPLAY: Symbol = canon(b'D', b'S', b'P');
pub const DRIVER_STORAGE: Symbol = canon(b'S', b'T', b'R');
pub const DRIVER_TIMER: Symbol = canon(b'T', b'M', b'R');
pub const DRIVER_OTHER: Symbol = canon(b'O', b'T', b'H');
pub const EMITS: Symbol = canon(b'E', b'M', b'T');
pub const DEVICE: Symbol = canon(b'D', b'E', b'V');
pub const FRAMEBUFFER_DEVICE: Symbol = canon(b'F', b'B', b'D');
pub const KEYBOARD_DEVICE: Symbol = canon(b'K', b'B', b'D');
pub const MOUSE_DEVICE: Symbol = canon(b'M', b'D', b'V');
pub const NIC_DEVICE: Symbol = canon(b'N', b'I', b'C');
pub const IRQ_EVENT: Symbol = canon(b'I', b'R', b'Q');
pub const DMA_EVENT: Symbol = canon(b'D', b'M', b'A');
pub const IRQ_LINE: Symbol = canon(b'I', b'Q', b'L');
pub const BUFFER: Symbol = canon(b'B', b'U', b'F');
pub const BYTES: Symbol = canon(b'B', b'Y', b'T');
pub const DONE: Symbol = canon(b'D', b'O', b'N');
pub const WINDOW_RECT: Symbol = canon(b'W', b'R', b'C');
pub const COLOR: Symbol = canon(b'C', b'L', b'R');
pub const VISIBLE: Symbol = canon(b'V', b'S', b'B');
pub const BITMAP: Symbol = canon(b'B', b'M', b'P');
pub const DIRTY: Symbol = canon(b'D', b'R', b'T');
pub const HAS_SURFACE: Symbol = canon(b'H', b'S', b'F');
pub const PRESENTS: Symbol = canon(b'P', b'R', b'S');

// Bundle types - used to classify bundles by their role
pub const APP: Symbol = canon(b'A', b'P', b'P');
pub const TYPE: Symbol = canon(b'T', b'Y', b'P');
pub const VERSION: Symbol = canon(b'V', b'E', b'R');

pub const INPUT_DEVICE_MOUSE: Symbol = canon(b'I', b'D', b'M');
pub const INPUT_EVENT: Symbol = canon(b'I', b'E', b'V');
pub const MOVE: Symbol = canon(b'M', b'O', b'V');
pub const DEVICE_ID: Symbol = canon(b'D', b'I', b'D');
pub const TS: Symbol = canon(b'T', b'S', b' ');
pub const DOWN: Symbol = canon(b'D', b'W', b'N');
pub const BUTTON: Symbol = canon(b'B', b'T', b'#');

pub fn from_str(s: &str) -> Option<Symbol> {
    const TBL: &[(Symbol, &str)] = &[
        (JOURNAL, "journal"),
        (KEYBOARD, "keyboard"),
        (KEY_PRESSED, "key_pressed"),
        (KEY_EVENT, "key_event"),
        (MOUSE, "mouse"),
        (MOUSE_MOVED, "mouse_moved"),
        (MOUSE_MOVE, "mouse_move"),
        (MOUSE_BUTTON, "mouse_button"),
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
        (BUNDLE, "bundle"),
        (OWNER, "owner"),
        (OWNS, "owns"),
        (CAN_READ, "can_read"),
        (CAN_WRITE, "can_write"),
        (CAN_LINK, "can_link"),
        (CAN_HANDLE_IRQ, "can_handle_irq"),
        (CAN_DMA, "can_dma"),
        (CAN_MMIO, "can_mmio"),
        (CAN_PORT_IO, "can_port_io"),
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
        (DEVICE, "device"),
        (FRAMEBUFFER_DEVICE, "device.framebuffer"),
        (KEYBOARD_DEVICE, "device.keyboard"),
        (MOUSE_DEVICE, "device.mouse"),
        (NIC_DEVICE, "device.nic"),
        (IRQ_EVENT, "irq.event"),
        (DMA_EVENT, "dma.event"),
        (IRQ_LINE, "irq_line"),
        (BUFFER, "buffer"),
        (BYTES, "bytes"),
        (DONE, "done"),
        (WINDOW_RECT, "window_rect"),
        (COLOR, "color"),
        (VISIBLE, "visible"),
        (BITMAP, "bitmap"),
        (DIRTY, "dirty"),
        (HAS_SURFACE, "has_surface"),
        (PRESENTS, "presents"),
        (SURFACE, "surface"),
        (CURSOR, "cursor"),
        (SHARED_BUFFER, "buffer.shared"),
        (QUEUE_STATE, "queue.state"),
        (BUFFER_KIND, "buffer.kind"),
        (BUFFER_USAGE, "buffer.usage"),
        (RING, "ring"),
        (LINEAR, "linear"),
        (PIPE_USAGE, "pipe"),
        (SURFACE_USAGE, "surface"),
        (RX_RING_USAGE, "rx_ring"),
        (HEAD, "head"),
        (TAIL, "tail"),
        (HAS_DATA, "has_data"),
        (CAPACITY, "capacity"),
        (X, "x"),
        (Y, "y"),
        (Z, "z"),
        (APP, "app"),
        (TYPE, "type"),
        (VERSION, "version"),
    ];
    let mut i = 0;
    while i < TBL.len() {
        if TBL[i].1 == s {
            return Some(TBL[i].0);
        }
        i += 1;
    }
    None
}

pub fn symbol_to_string(sym: Symbol) -> String {
    const TBL: &[(Symbol, &str)] = &[
        (JOURNAL, "journal"),
        (KEYBOARD, "keyboard"),
        (KEY_PRESSED, "key_pressed"),
        (KEY_EVENT, "key_event"),
        (MOUSE, "mouse"),
        (MOUSE_MOVED, "mouse_moved"),
        (MOUSE_MOVE, "mouse_move"),
        (MOUSE_BUTTON, "mouse_button"),
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
        (BUNDLE, "bundle"),
        (OWNER, "owner"),
        (OWNS, "owns"),
        (CAN_READ, "can_read"),
        (CAN_WRITE, "can_write"),
        (CAN_LINK, "can_link"),
        (CAN_HANDLE_IRQ, "can_handle_irq"),
        (CAN_DMA, "can_dma"),
        (CAN_MMIO, "can_mmio"),
        (CAN_PORT_IO, "can_port_io"),
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
        (DEVICE, "device"),
        (FRAMEBUFFER_DEVICE, "device.framebuffer"),
        (KEYBOARD_DEVICE, "device.keyboard"),
        (MOUSE_DEVICE, "device.mouse"),
        (NIC_DEVICE, "device.nic"),
        (IRQ_EVENT, "irq.event"),
        (DMA_EVENT, "dma.event"),
        (IRQ_LINE, "irq_line"),
        (BUFFER, "buffer"),
        (BYTES, "bytes"),
        (DONE, "done"),
        (WINDOW_RECT, "window_rect"),
        (COLOR, "color"),
        (VISIBLE, "visible"),
        (BITMAP, "bitmap"),
        (DIRTY, "dirty"),
        (HAS_SURFACE, "has_surface"),
        (PRESENTS, "presents"),
        (SURFACE, "surface"),
        (CURSOR, "cursor"),
        (SHARED_BUFFER, "buffer.shared"),
        (QUEUE_STATE, "queue.state"),
        (BUFFER_KIND, "buffer.kind"),
        (BUFFER_USAGE, "buffer.usage"),
        (RING, "ring"),
        (LINEAR, "linear"),
        (PIPE_USAGE, "pipe"),
        (SURFACE_USAGE, "surface"),
        (RX_RING_USAGE, "rx_ring"),
        (HEAD, "head"),
        (TAIL, "tail"),
        (HAS_DATA, "has_data"),
        (CAPACITY, "capacity"),
        (X, "x"),
        (Y, "y"),
        (Z, "z"),
        (APP, "app"),
        (TYPE, "type"),
        (VERSION, "version"),
    ];
    for (s, name) in TBL {
        if *s == sym {
            return name.to_ascii_uppercase();
        }
    }
    let val = sym.0;
    let c1 = ((val >> 16) & 0xFF) as u8;
    let c2 = ((val >> 8) & 0xFF) as u8;
    let c3 = (val & 0xFF) as u8;
    let mut bytes = Vec::new();
    if c1 != 0 { bytes.push(c1); }
    if c2 != 0 { bytes.push(c2); }
    if c3 != 0 { bytes.push(c3); }
    if let Ok(s) = String::from_utf8(bytes) {
        if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
             return s.to_ascii_uppercase();
        }
    }
    alloc::format!("SYM_{}", val)
}
