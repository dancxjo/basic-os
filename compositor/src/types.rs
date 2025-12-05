use alloc::string::String;
use alloc::string::ToString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn union(&self, other: Rect) -> Rect {
        use core::cmp::{max, min};
        let x = min(self.x, other.x);
        let y = min(self.y, other.y);
        let max_x = max(self.x + self.width as i32, other.x + other.width as i32);
        let max_y = max(self.y + self.height as i32, other.y + other.height as i32);
        Rect::new(x, y, (max_x - x) as u32, (max_y - y) as u32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(a: u8, r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0xff }
    }

    pub const fn to_u32(self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub frame_outer: Rgba,
    pub frame_light: Rgba,
    pub frame_hilight: Rgba,
    pub frame_shadow: Rgba,
    pub title_active: Rgba,
    pub title_inactive: Rgba,
    pub title_text_active: Rgba,
    pub title_text_inactive: Rgba,
    pub client_bg: Rgba,
    pub inactive_veil: Rgba,
}

pub const THEME: Theme = Theme {
    frame_outer: Rgba::new(0xff, 0x5A, 0x6A, 0x8A),
    frame_light: Rgba::new(0xff, 0xE6, 0xED, 0xF7),
    frame_hilight: Rgba::new(0xff, 0xFF, 0xFF, 0xFF),
    frame_shadow: Rgba::new(0xff, 0x9A, 0xA7, 0xC5),
    title_active: Rgba::new(0xff, 0x6C, 0x9A, 0xFF),
    title_inactive: Rgba::new(0xff, 0xE3, 0xEA, 0xF8),
    title_text_active: Rgba::new(0xff, 0x10, 0x1F, 0x3F),
    title_text_inactive: Rgba::new(0xff, 0x6A, 0x74, 0x8A),
    client_bg: Rgba::new(0xff, 0xFD, 0xFB, 0xF7),
    inactive_veil: Rgba::new(0x80, 0x00, 0x00, 0x00),
};

pub const FONT_HEIGHT: usize = 16;
pub const TITLE_BAR_HEIGHT: usize = 32;
pub const BORDER_OUTER_THICKNESS: i32 = 1;
pub const BORDER_3D_THICKNESS: i32 = 1;
pub const BORDER_THICKNESS: i32 = BORDER_OUTER_THICKNESS + BORDER_3D_THICKNESS;
pub const RESIZE_MARGIN: i32 = 6;
pub const RESIZE_CORNER_SIZE: i32 = 8;
pub const MIN_WINDOW_WIDTH: i32 = 140;
pub const MIN_WINDOW_HEIGHT: i32 = 100;
pub const CLOSE_BUTTON_SIZE: i32 = 28;
pub const CLOSE_BUTTON_MARGIN_RIGHT: i32 = 2;
pub const CLOSE_BUTTON_MARGIN_TOP: i32 = 2;
pub const TITLE_TEXT_LEFT_PAD: i32 = 8;
pub const TITLE_TEXT_TOP_OFFSET: i32 = 8;
pub const TOOLBAR_HEIGHT: i32 = 32;
pub const TOOLBAR_BUTTON_SIZE: i32 = 24;
pub const TOOLBAR_BUTTON_SPACING: i32 = 2;
pub const CURSOR_SIZE: usize = 98;
pub const SCROLLBAR_WIDTH: i32 = 24;
pub const SCROLLBAR_GAP: i32 = 4;
pub const SCROLLBAR_MIN_THUMB: i32 = 32;
pub const SCROLL_STEP_LINE: i32 = FONT_HEIGHT as i32;
pub const SCROLLBAR_TOTAL_RESERVE: i32 = SCROLLBAR_WIDTH + SCROLLBAR_GAP;
pub const AUTO_TILE_MARGIN: i32 = 8;
pub const AUTO_TILE_MIN_WINDOWS: usize = 3;
pub const AUTO_TILE_TOP_OFFSET: i32 = 48;

pub const ROLE_TOOLBAR: &str = "container.toolbar";
pub const ROLE_TOOLBAR_BUTTON: &str = "control.toolbar_button";
pub const ROLE_CONTAINER_VERTICAL: &str = "container.vertical";
pub const ROLE_EDITOR_ROOT: &str = "container.editor_root";

// Buttons
pub const BTN_FACE: Rgba = Rgba::new(0xff, 0xE6, 0xED, 0xF7);
pub const BTN_BORDER: Rgba = Rgba::new(0xff, 0x5A, 0x6A, 0x8A);
pub const BTN_GLYPH: Rgba = Rgba::new(0xff, 0xB8, 0x51, 0x51);
// Scrollbar colors
pub const SCROLLBAR_TRACK_COLOR: Rgba = Rgba::new(0xff, 0xE2, 0xE6, 0xF0);
pub const SCROLLBAR_THUMB_COLOR: Rgba = Rgba::new(0xff, 0x7C, 0x8B, 0xAB);
pub const SCROLLBAR_THUMB_HILIGHT: Rgba = Rgba::new(0xff, 0xF5, 0xF7, 0xFB);
pub const SCROLLBAR_THUMB_SHADOW: Rgba = Rgba::new(0xff, 0x4A, 0x54, 0x6A);

pub const COLOR_TEXT: Rgba = THEME.title_text_active;
pub const COLOR_CURSOR_PRIMARY: Rgba = Rgba::new(0xff, 0xff, 0xff, 0xff);
pub const COLOR_CURSOR_SHADOW: Rgba = Rgba::new(0x40, 0x00, 0x00, 0x00);
pub const CLEAR_COLOR: Rgba = Rgba::new(0xff, 0x00, 0x00, 0x00);

pub fn clamp_i32(v: i32, min_v: i32, max_v: i32) -> i32 {
    if v < min_v {
        min_v
    } else if v > max_v {
        max_v
    } else {
        v
    }
}
