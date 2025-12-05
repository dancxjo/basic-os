use alloc::vec::Vec;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ModeSlot {
    pub mode_index: u8,
    pub root_window: Option<Uuid>,
    pub windows: Vec<Uuid>,
    pub place_id: Option<Uuid>,
}

impl ModeSlot {
    pub fn new(index: u8) -> Self {
        Self {
            mode_index: index,
            root_window: None,
            windows: Vec::new(),
            place_id: None,
        }
    }
}
