//! Symbol table helpers. Keep codes short and human-readable for dumps.

use alloc::string::String;
use alloc::vec::Vec;
#[allow(unused_imports)] // Re-exported for downstream modules.
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

// Graph Policy Symbols
pub const PUBLIC: Symbol = canon(b'P', b'U', b'B');
pub const CLOSE_WINDOW_ACTION: Symbol = canon(b'C', b'L', b'W');

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
pub const TEXT: Symbol = canon(b'T', b'X', b'T');
pub const TARGET: Symbol = canon(b'T', b'G', b'T');
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

pub const DRIVER_INPUT: Symbol = canon(b'I', b'N', b'P');
pub const DRIVER_DISPLAY: Symbol = canon(b'D', b'S', b'P');
pub const DRIVER_STORAGE: Symbol = canon(b'S', b'T', b'R');
pub const DRIVER_TIMER: Symbol = canon(b'T', b'M', b'R');
pub const DRIVER_OTHER: Symbol = canon(b'O', b'T', b'H');

pub const WINDOW: Symbol = canon(b'W', b'I', b'N');
pub const PIXMAP: Symbol = canon(b'P', b'X', b'M');
pub const STREAMS: Symbol = canon(b'S', b'T', b'M');
pub const COMPOSED_BY: Symbol = canon(b'C', b'M', b'B');
pub const WINDOW_CREATED: Symbol = canon(b'W', b'C', b'R');
pub const WINDOW_BUFFER_UPDATED: Symbol = canon(b'W', b'B', b'U');
pub const FRAME_READY: Symbol = canon(b'F', b'R', b'M');
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

pub const INPUT_DEVICE_MOUSE: Symbol = canon(b'I', b'D', b'M');
pub const INPUT_EVENT: Symbol = canon(b'I', b'E', b'V');
pub const MOVE: Symbol = canon(b'M', b'O', b'V');
pub const DEVICE_ID: Symbol = canon(b'D', b'I', b'D');
pub const TS: Symbol = canon(b'T', b'S', b' ');
pub const DOWN: Symbol = canon(b'D', b'W', b'N');
pub const BUTTON: Symbol = canon(b'B', b'T', b'#');

pub const MODE: Symbol = canon(b'M', b'O', b'D');
pub const PACKAGE: Symbol = canon(b'P', b'K', b'G');
pub const TASK: Symbol = canon(b'T', b'S', b'K');
pub const DRIVES: Symbol = canon(b'D', b'R', b'V');
pub const INSTANCE_OF: Symbol = canon(b'I', b'N', b'S');
pub const BOOT_ASSET: Symbol = canon(b'B', b'T', b'A');
pub const LAUNCH_INTENT: Symbol = canon(b'L', b'N', b'C');
pub const AUTOSTART: Symbol = canon(b'A', b'U', b'T');
pub const SHOW_IN_GRAPH_VIEWER: Symbol = canon(b'S', b'I', b'G');
pub const BIN_NAME: Symbol = canon(b'B', b'N', b'M');
pub const SURFACE: Symbol = canon(b'S', b'F', b'C');
pub const CURSOR: Symbol = canon(b'C', b'R', b'S');
pub const TITLE: Symbol = canon(b'T', b'T', b'L');
pub const GAP: Symbol = canon(b'G', b'A', b'P');
pub const IS_ROOT: Symbol = canon(b'I', b'S', b'R');
pub const MODE_INDEX: Symbol = canon(b'M', b'D', b'X');
pub const WINDOW_RECT: Symbol = canon(b'W', b'R', b'C');
pub const TOOLBAR: Symbol = canon(b'T', b'L', b'B');
pub const TOOLBAR_BUTTON: Symbol = canon(b'T', b'B', b'T');
pub const ICON: Symbol = canon(b'I', b'C', b'N');
pub const ACTIVATED: Symbol = canon(b'A', b'C', b'T');
pub const SCROLL_Y: Symbol = canon(b'S', b'C', b'Y');
pub const MAX_SCROLL: Symbol = canon(b'M', b'X', b'S');
pub const CONTENT_HEIGHT: Symbol = canon(b'C', b'T', b'H');
pub const THUMB_OFFSET: Symbol = canon(b'T', b'O', b'F');
pub const THUMB_HEIGHT: Symbol = canon(b'T', b'H', b'T');
pub const MIN_WIDTH: Symbol = canon(b'M', b'N', b'W');
pub const MAX_WIDTH: Symbol = canon(b'M', b'X', b'W');
pub const MIN_HEIGHT: Symbol = canon(b'M', b'N', b'H');
pub const MAX_HEIGHT: Symbol = canon(b'M', b'X', b'H');
pub const VIEWPORT_HEIGHT: Symbol = canon(b'V', b'P', b'H');
pub const PARENT: Symbol = canon(b'P', b'A', b'R');
pub const BUNDLE_ID: Symbol = canon(b'B', b'I', b'D');
pub const DEVICE_DRIVER: Symbol = canon(b'D', b'D', b'R');
pub const DIRECTORY: Symbol = canon(b'D', b'I', b'R');
pub const FILE: Symbol = canon(b'F', b'I', b'L');
pub const APP: Symbol = canon(b'A', b'P', b'P');
pub const TYPE: Symbol = canon(b'T', b'Y', b'P');
pub const VERSION: Symbol = canon(b'V', b'E', b'R');

pub const UP: Symbol = canon(b'U', b'P', b' ');
pub const DISPLAY_FRAMEBUFFER: Symbol = canon(b'D', b'F', b'B');
pub const DISPLAY_FRAME: Symbol = canon(b'D', b'F', b'R');
pub const CURRENT_FRAME: Symbol = canon(b'C', b'U', b'R');
pub const SEQ: Symbol = canon(b'S', b'E', b'Q');
pub const HAS_CONTENT: Symbol = canon(b'H', b'C', b'T');
pub const COLOR: Symbol = canon(b'C', b'L', b'R');
pub const VISIBLE: Symbol = canon(b'V', b'S', b'B');
pub const ACTIVE: Symbol = canon(b'A', b'C', b'T');
pub const BITMAP: Symbol = canon(b'B', b'M', b'P');
pub const DIRTY: Symbol = canon(b'D', b'R', b'T');
pub const HAS_SURFACE: Symbol = canon(b'H', b'S', b'F');
pub const PRESENTS: Symbol = canon(b'P', b'R', b'S');
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
pub const CLOUD: Symbol = canon(b'C', b'L', b'D');
pub const NEXT: Symbol = cc('N', 'X');
pub const LAUNCH_REQUEST: Symbol = canon(b'L', b'R', b'Q');
pub const CAN_EDIT: Symbol = canon(b'C', b'E', b'D');
pub const LAYOUT_TEMPLATE: Symbol = canon(b'L', b'T', b'P');
pub const LAYOUT_REGION: Symbol = canon(b'L', b'R', b'G');
pub const CONTAINS: Symbol = canon(b'C', b'O', b'N');
pub const BINDS_TO: Symbol = canon(b'B', b'N', b'T');
pub const FLEX_DIRECTION: Symbol = cc('F', 'D');
pub const FLEX_GROW: Symbol = cc('F', 'G');
pub const FLEX_SHRINK: Symbol = cc('F', 'S');
pub const FOR_DOCUMENT: Symbol = canon(b'F', b'D', b'C');
pub const HANDLED_BY: Symbol = canon(b'H', b'D', b'B');
pub const REQUESTS: Symbol = canon(b'R', b'Q', b'S');
pub const SAVE_EVENT: Symbol = canon(b'S', b'A', b'V');
pub const APPLIES_TO: Symbol = canon(b'A', b'P', b'L');
pub const ROLE: Symbol = canon(b'R', b'O', b'L');
pub const WIDGET: Symbol = canon(b'W', b'G', b'T');
pub const WIDGET_KIND: Symbol = canon(b'W', b'K', b'D');
pub const ICON_NAME: Symbol = canon(b'I', b'C', b'N');
pub const FOCUSABLE: Symbol = canon(b'F', b'O', b'C');
pub const BINDS: Symbol = canon(b'B', b'N', b'D');
pub const ABOVE: Symbol = canon(b'A', b'B', b'V');
pub const ACTIVE_WINDOW: Symbol = canon(b'A', b'C', b'W');
pub const SELECTED_INDEX: Symbol = canon(b'S', b'L', b'X');
pub const DOCUMENT: Symbol = canon(b'D', b'O', b'C');
pub const VIEW: Symbol = canon(b'V', b'I', b'W');
pub const WIDGET_ROLE: Symbol = canon(b'W', b'R', b'L');
pub const ACTION: Symbol = canon(b'A', b'C', b'N');
pub const LABEL: Symbol = canon(b'L', b'B', b'L');
pub const DESCRIPTION: Symbol = canon(b'D', b'S', b'C');
pub const FOCUSED: Symbol = canon(b'F', b'C', b'D');
pub const TAB_INDEX: Symbol = canon(b'T', b'B', b'I');
pub const CHILD: Symbol = canon(b'C', b'H', b'D');
pub const LABEL_FOR: Symbol = canon(b'L', b'B', b'F');
pub const HAS_CURSOR: Symbol = canon(b'H', b'C', b'R');
pub const CONTROLS: Symbol = canon(b'C', b'T', b'L');
pub const PAGE_SIZE: Symbol = canon(b'P', b'G', b'S');
pub const TOTAL_SIZE: Symbol = canon(b'T', b'T', b'S');
pub const POSITION: Symbol = canon(b'P', b'O', b'S');
pub const ORIENTATION: Symbol = canon(b'O', b'R', b'N');
pub const MIME: Symbol = canon(b'M', b'I', b'M');
pub const ENCODING: Symbol = canon(b'E', b'N', b'C');
pub const LENGTH: Symbol = canon(b'L', b'E', b'N');
pub const CURRENT_MODE: Symbol = canon(b'C', b'M', b'D');
pub const PLACE: Symbol = canon(b'P', b'L', b'C');
pub const PLACE_NAME: Symbol = canon(b'P', b'N', b'M');
pub const PLACE_PACKAGE: Symbol = canon(b'P', b'P', b'K');
pub const PLACE_ROOT_KIND: Symbol = canon(b'P', b'R', b'K');
pub const PLACE_FULLSCREEN: Symbol = canon(b'P', b'F', b'S');
pub const MODE_PLACE: Symbol = canon(b'M', b'D', b'P');
pub const IS_PLACE_ROOT: Symbol = canon(b'I', b'P', b'R');
pub const LAUNCHED: Symbol = canon(b'L', b'C', b'H');
pub const WALLPAPER: Symbol = canon(b'W', b'L', b'P');
pub const LAYER: Symbol = canon(b'L', b'Y', b'R');
pub const SOLID_COLOR: Symbol = canon(b'S', b'L', b'C');
pub const IMAGE: Symbol = canon(b'I', b'M', b'G');
pub const SCROLL_X: Symbol = canon(b'S', b'C', b'X');
pub const OPACITY: Symbol = canon(b'O', b'P', b'C');
pub const Z_ORDER: Symbol = canon(b'Z', b'O', b'R');
pub const TILE_MODE: Symbol = canon(b'T', b'L', b'M');
pub const LAYERS: Symbol = canon(b'L', b'Y', b'S');
pub const ROOT_LAYOUT: Symbol = canon(b'R', b'L', b'Y');
pub const INDEX: Symbol = canon(b'I', b'D', b'X');
pub const OF: Symbol = cc('O', 'F');
pub const SHOWS: Symbol = canon(b'S', b'H', b'W');
pub const EDITED_BY: Symbol = canon(b'E', b'D', b'B');
pub const EDIT: Symbol = canon(b'E', b'D', b'T');
pub const READ_ONLY: Symbol = canon(b'R', b'D', b'O');
pub const CONTROL: Symbol = canon(b'C', b'T', b'R');
pub const CONTROL_KIND: Symbol = canon(b'C', b'T', b'K');
pub const QUESTION: Symbol = canon(b'Q', b'U', b'E');
pub const ANSWER: Symbol = canon(b'A', b'N', b'S');
pub const FORM: Symbol = canon(b'F', b'O', b'R');
pub const HAS_ANSWER: Symbol = canon(b'H', b'A', b'N');
pub const FORM_CONTAINS: Symbol = canon(b'F', b'C', b'N');
pub const ANSWER_KIND: Symbol = canon(b'A', b'K', b'D');
pub const VALUE_BOOL: Symbol = canon(b'V', b'B', b'L');
pub const VALUE_TEXT: Symbol = canon(b'V', b'T', b'X');
pub const TAG: Symbol = canon(b'T', b'A', b'G');
pub const SHOW_TAG: Symbol = canon(b'S', b'H', b'T');
pub const PREFERRED_WIDGET: Symbol = canon(b'P', b'W', b'G');
pub const VALUE_NUMBER: Symbol = canon(b'V', b'N', b'M');
pub const INTERACTION: Symbol = canon(b'I', b'T', b'N');
pub const INTERACTION_KIND: Symbol = canon(b'I', b'K', b'D');
pub const USES_WIDGET: Symbol = canon(b'U', b'W', b'D');
pub const RESULT_WRITES: Symbol = canon(b'R', b'S', b'W');
pub const RESULT_EXECUTES: Symbol = canon(b'R', b'S', b'E');
pub const FOCUSED_INDEX: Symbol = canon(b'F', b'O', b'C');
pub const THEME: Symbol = canon(b'T', b'H', b'M');
pub const WIDGET_KIND_LISTBOX: Symbol = canon(b'W', b'L', b'B');
pub const NOTIFICATION: Symbol = canon(b'N', b'T', b'F');
pub const ALERT: Symbol = canon(b'A', b'L', b'R');
pub const LEVEL: Symbol = canon(b'L', b'V', b'L');
pub const MESSAGE: Symbol = canon(b'M', b'S', b'G');
pub const DETAILS: Symbol = canon(b'D', b'T', b'L');
pub const CREATED_AT: Symbol = canon(b'C', b'A', b'T');
pub const ACK: Symbol = canon(b'A', b'C', b'K');
pub const SCOPE: Symbol = canon(b'S', b'C', b'P');
pub const PERSIST: Symbol = canon(b'P', b'S', b'T');
pub const WIDGET_KIND_NOTIFICATION: Symbol = canon(b'W', b'N', b'T');
pub const THING_WIDGET: Symbol = canon(b'T', b'H', b'W');
pub const STYLE: Symbol = canon(b'S', b'T', b'Y');
pub const DEBUG_LOG: Symbol = canon(b'D', b'B', b'G');

pub fn from_str(s: &str) -> Option<Symbol> {
    const TBL: &[(Symbol, &str)] = &[
        (JOURNAL, "journal"),
        (STYLE, "style"),
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
        (BOOT_ASSET, "boot.asset"),
        (PUBLIC, "public"),
        (CLOSE_WINDOW_ACTION, "action.close_window"),
        (STYLE, "style"),
    ];
    let mut i = 0;
    while i < TBL.len() {
        if TBL[i].1 == s {
            return Some(TBL[i].0);
        }
        i += 1;
    }
    if s.len() == 3 {
        let b = s.as_bytes();
        return Some(canon(b[0], b[1], b[2]));
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
        (PUBLIC, "public"),
        (CLOSE_WINDOW_ACTION, "action.close_window"),
        (STYLE, "style"),
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
    if c1 != 0 {
        bytes.push(c1);
    }
    if c2 != 0 {
        bytes.push(c2);
    }
    if c3 != 0 {
        bytes.push(c3);
    }
    if let Ok(s) = String::from_utf8(bytes) {
        if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return s.to_ascii_uppercase();
        }
    }
    alloc::format!("SYM_{}", val)
}
