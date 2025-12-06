use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::bitmap::Bitmap;
use crate::types::{Rect, Rgba};

#[derive(Clone, Debug)]
pub enum SceneItem {
    Clear {
        color: Rgba,
    },
    FillRect {
        rect: Rect,
        color: Rgba,
    },
    HatchRect {
        rect: Rect,
        color: Rgba,
        spacing: i32,
    },
    BlitImage {
        rect: Rect,
        image: Arc<Bitmap>,
        repeat: bool,
        offset: (i32, i32),
    },
    DrawText {
        origin: (i32, i32),
        text: String,
        color: Rgba,
        max_width: Option<u32>,
    },
    DrawTextBlock {
        rect: Rect,
        text: String,
        color: Rgba,
        scroll_offset: i32,
    },
    DrawCursor {
        origin: (i32, i32),
        sprite: Arc<Bitmap>,
        hotspot: (i32, i32),
    },
    DrawLine {
        start: (i32, i32),
        end: (i32, i32),
        color: Rgba,
    },
    ClipPush {
        rect: Rect,
    },
    ClipPop,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub width: u32,
    pub height: u32,
    pub items: Vec<SceneItem>,
}

impl Scene {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            items: Vec::new(),
        }
    }

    pub fn push(&mut self, item: SceneItem) {
        self.items.push(item);
    }

    pub fn items(&self) -> &[SceneItem] {
        &self.items
    }
}
