use alloc::vec::Vec;
use uuid::Uuid;

pub struct Screen {
    pub width: u32,
    pub height: u32,
    pub background_id: Option<Uuid>, // "tiles" predicate
    pub overlays: Vec<Uuid>,         // e.g., windows, cursors, log views, etc.
}

impl Screen {
    pub fn new(width: u32, height: u32) -> Self {
        Screen {
            width,
            height,
            background_id: None,
            overlays: Vec::new(),
        }
    }

    // fn background(&self) -> Option<&[u8]> {
    //     let screen = kernel
    //         .graph
    //         .find_one_thing::<Screen>()
    //         .expect("Screen not found");
    //     let id = screen.uuid.clone();
    //     let background = kernel.graph.get_thing_that::<&[u8]>("tiles", id);
    // }
}
