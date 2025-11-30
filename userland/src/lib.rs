#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]

extern crate alloc;

pub mod app;
pub mod drivers;
pub mod graph;
pub mod graphics;
pub mod heap;
pub mod runtime;
pub mod semantic_ui;
pub mod sys;
pub mod watch;

pub use heap::init_heap;
#[cfg(feature = "std")]
pub use runtime::host_runtime;
pub use runtime::{ensure_kernel_runtime, runtime, set_runtime};
pub use thing_abi::{
    AbiRequest, AbiResponse, FramebufferGeometry, GrantCapabilityRequest, Map, Symbol,
    ThingRuntime, Value,
};
pub use uuid;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::sys::print_fmt(core::format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {
        $crate::print!("{}\n", core::format_args!($($arg)*));
    };
}

pub mod canon {
    pub use thing_abi::{canon, cc, from_char, from_u16, Symbol};
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
    pub const DRIVER_INPUT: Symbol = canon(b'I', b'N', b'P');
    pub const DRIVER_DISPLAY: Symbol = canon(b'D', b'S', b'P');
    pub const DRIVER_STORAGE: Symbol = canon(b'S', b'T', b'R');
    pub const DRIVER_TIMER: Symbol = canon(b'T', b'M', b'R');
    pub const DRIVER_OTHER: Symbol = canon(b'O', b'T', b'H');
    pub const TITLE: Symbol = canon(b'T', b'T', b'L');
    pub const X: Symbol = canon(b'X', b' ', b' ');
    pub const Y: Symbol = canon(b'Y', b' ', b' ');
    pub const Z: Symbol = canon(b'Z', b'I', b'N');
    pub const WIDTH: Symbol = canon(b'W', b'D', b'T');
    pub const HEIGHT: Symbol = canon(b'H', b'G', b'T');
    pub const PITCH: Symbol = canon(b'P', b'T', b'H');
    pub const BPP: Symbol = canon(b'B', b'P', b'P');
    pub const ADDR: Symbol = canon(b'A', b'D', b'R');
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

    pub const DISPLAY_FRAMEBUFFER: Symbol = canon(b'D', b'F', b'B');
    pub const DISPLAY_FRAME: Symbol = canon(b'D', b'F', b'R');
    pub const CURRENT_FRAME: Symbol = canon(b'C', b'U', b'R');
    pub const SEQ: Symbol = canon(b'S', b'E', b'Q');
    pub const APP: Symbol = canon(b'A', b'P', b'P');
    pub const BUNDLE: Symbol = canon(b'B', b'N', b'D');
    pub const TYPE: Symbol = canon(b'T', b'Y', b'P');
    pub const VERSION: Symbol = canon(b'V', b'E', b'R');
    pub const OWNER: Symbol = canon(b'O', b'W', b'N');
    pub const OWNS: Symbol = canon(b'O', b'W', b'S');
    pub const HAS_CONTENT: Symbol = canon(b'H', b'C', b'T');
    pub const WINDOW_RECT: Symbol = canon(b'W', b'R', b'C');
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

    // Capability symbols for granting permissions
    pub const CAN_READ: Symbol = canon(b'C', b'R', b'D');
    pub const CAN_WRITE: Symbol = canon(b'C', b'W', b'R');
    pub const CAN_LINK: Symbol = canon(b'C', b'L', b'K');
    pub const CAN_HANDLE_IRQ: Symbol = canon(b'C', b'I', b'Q');
    pub const CAN_DMA: Symbol = canon(b'C', b'D', b'M');
    pub const CAN_MMIO: Symbol = canon(b'C', b'M', b'M');
    pub const CLOUD: Symbol = canon(b'C', b'L', b'D');
    pub const NEXT: Symbol = cc('N', 'X');

    pub const PACKAGE: Symbol = canon(b'P', b'K', b'G');
    pub const TASK: Symbol = canon(b'T', b'S', b'K');
    pub const LAUNCH_REQUEST: Symbol = canon(b'L', b'R', b'Q');
    pub const CAN_EDIT: Symbol = canon(b'C', b'E', b'D');
    pub const DRIVES: Symbol = canon(b'D', b'R', b'V');
    pub const ABOVE: Symbol = canon(b'A', b'B', b'V');
    pub const ACTIVE_WINDOW: Symbol = canon(b'A', b'C', b'W');
    pub const FOR_DOCUMENT: Symbol = canon(b'F', b'D', b'C');
    pub const HANDLED_BY: Symbol = canon(b'H', b'D', b'B');
    pub const REQUESTS: Symbol = canon(b'R', b'Q', b'S');
    pub const SAVE_EVENT: Symbol = canon(b'S', b'A', b'V');
    pub const APPLIES_TO: Symbol = canon(b'A', b'P', b'L');

    // Document & Editor symbols
    pub const DOCUMENT: Symbol = canon(b'D', b'O', b'C');
    pub const VIEW: Symbol = canon(b'V', b'I', b'W');
    pub const WIDGET: Symbol = canon(b'W', b'D', b'G');
    pub const ROLE: Symbol = canon(b'R', b'O', b'L');
    pub const LABEL: Symbol = canon(b'L', b'B', b'L');
    pub const DESCRIPTION: Symbol = canon(b'D', b'S', b'C');
    pub const FOCUSABLE: Symbol = canon(b'F', b'C', b'S');
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
    pub const MODE: Symbol = canon(b'M', b'O', b'D');
    pub const OF: Symbol = cc('O', 'F');
    pub const SHOWS: Symbol = canon(b'S', b'H', b'W');
    pub const EDITED_BY: Symbol = canon(b'E', b'D', b'B');
    pub const EDIT: Symbol = canon(b'E', b'D', b'T');
    pub const READ_ONLY: Symbol = canon(b'R', b'D', b'O');
}

pub mod prelude {
    pub use crate::app::{App, AppContext, WindowHandle};
    pub use crate::app_main;
    pub use crate::graph::{
        declare_queue_state, declare_shared_buffer, extract_text, fiat, fiat_thing, find_by_kind,
        grant_capability, load_thing, load_things_of_kind, map, that, update_queue_state,
        update_thing, QueueState, SharedBuffer, Surface, Thingable, Window,
    };
    pub use crate::watch::{AppEvent, EventFilter, ThingFilter, WatchId, WatchManager};
    pub use crate::{print, println, Value};
}

pub use app::{App, AppContext, AppRunner, DynApp, WindowHandle};
pub use graph::{
    declare_queue_state, declare_shared_buffer, extract_text, fiat, fiat_thing, find_by_kind,
    grant_capability, load_thing, load_things_of_kind, map, that, update_queue_state, update_thing,
    GraphEdge, GraphThing, NodePattern, QueueState, SharedBuffer, Surface, Thingable, Window,
};
pub use watch::{AppEvent, EventFilter, ThingFilter, WatchId, WatchManager};

use uuid::Uuid;

pub fn simple_uuid(name: &[u8]) -> Uuid {
    let mut hash = 0xcbf29ce484222325u64;
    for b in name {
        hash = hash ^ (*b as u64);
        hash = hash.wrapping_mul(0x1099511628211u64);
    }
    let u = ((hash as u128) << 64) | (hash as u128);
    Uuid::from_u128(u)
}
