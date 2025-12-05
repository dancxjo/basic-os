use alloc::string::String;
use alloc::sync::Arc;
use uuid::Uuid;
use widget_button::State as ButtonState;

use crate::bitmap::Bitmap;
use crate::types::{
    clamp_i32, Rect, BORDER_THICKNESS, CLOSE_BUTTON_MARGIN_RIGHT, CLOSE_BUTTON_MARGIN_TOP,
    CLOSE_BUTTON_SIZE, FONT_HEIGHT, RESIZE_CORNER_SIZE, RESIZE_MARGIN, SCROLLBAR_GAP,
    SCROLLBAR_MIN_THUMB, SCROLLBAR_TOTAL_RESERVE, SCROLLBAR_WIDTH, TITLE_BAR_HEIGHT,
};
use unifont::get_glyph;
use userland::Window;

#[derive(Clone, Debug, Default)]
pub struct Caret {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub visible: bool,
}

#[derive(Clone)]
pub struct WindowSurface {
    pub window: Window,
    pub surface_id: Option<Uuid>,
    pub text: String,
    pub bitmap: Option<Arc<Bitmap>>,
    pub scroll_y: i32,
    pub scrollbar_widget_id: Option<Uuid>,
    pub caret: Caret,
    pub close_button_id: Uuid,
    pub close_button_state: ButtonState,
    pub close_button_bitmap: Option<Arc<Bitmap>>,
}

pub struct WindowLayout {
    pub title_x: i32,
    pub title_y: i32,
    pub title_w: i32,
    pub title_h: i32,
    pub client_x: i32,
    pub client_y: i32,
    pub client_w: i32,
    pub client_h: i32,
}

#[derive(Clone, Debug)]
pub struct ContentMetrics {
    pub content_rect: Rect,
    pub viewport_height: i32,
    pub content_height: i32,
    pub max_scroll: i32,
    pub scroll_offset: i32,
    pub scrollbar_track_rect: Option<Rect>,
    pub scrollbar_thumb_rect: Option<Rect>,
    pub scrollbar_thumb_offset: Option<i32>,
}

pub fn compute_window_layout(
    win_x: i32,
    win_y: i32,
    win_w: i32,
    win_h: i32,
) -> Option<WindowLayout> {
    let border = BORDER_THICKNESS;
    let inner_w = win_w - border * 2;
    let inner_h = win_h - border * 2;
    if inner_w <= 0 || inner_h <= TITLE_BAR_HEIGHT as i32 {
        return None;
    }

    let title_x = win_x + border;
    let title_y = win_y + border;
    let title_w = inner_w;
    let title_h = TITLE_BAR_HEIGHT as i32;

    let client_x = title_x;
    let client_y = title_y + title_h;
    let client_w = inner_w;
    let client_h = inner_h - title_h;

    if client_w <= 0 || client_h <= 0 {
        return None;
    }

    Some(WindowLayout {
        title_x,
        title_y,
        title_w,
        title_h,
        client_x,
        client_y,
        client_w,
        client_h,
    })
}

impl ContentMetrics {
    pub fn new(surface: &WindowSurface, layout: &WindowLayout) -> Self {
        let viewport_height = (layout.client_h - 1).max(0);
        let client_y = layout.client_y + 1;
        let mut available_width = layout.client_w;
        if available_width <= 0 || viewport_height <= 0 {
            return Self {
                content_rect: Rect::new(layout.client_x, client_y, 0, viewport_height as u32),
                viewport_height,
                content_height: 0,
                max_scroll: 0,
                scroll_offset: 0,
                scrollbar_track_rect: None,
                scrollbar_thumb_rect: None,
                scrollbar_thumb_offset: None,
            };
        }

        let mut content_height = measure_surface_content_height(surface, available_width);
        let mut reserve_scrollbar = false;
        if content_height > viewport_height && available_width > SCROLLBAR_TOTAL_RESERVE {
            let candidate_width = available_width - SCROLLBAR_TOTAL_RESERVE;
            if candidate_width > 0 {
                let candidate_height = measure_surface_content_height(surface, candidate_width);
                if candidate_height > viewport_height {
                    available_width = candidate_width;
                    content_height = candidate_height;
                    reserve_scrollbar = true;
                }
            }
        }

        let max_scroll = content_height.saturating_sub(viewport_height).max(0);
        let clamped_scroll = clamp_i32(surface.scroll_y, 0, max_scroll);
        let content_rect = Rect::new(
            layout.client_x,
            client_y,
            available_width.max(0) as u32,
            viewport_height.max(0) as u32,
        );

        let mut scrollbar_track_rect = None;
        let mut scrollbar_thumb_rect = None;
        let mut scrollbar_thumb_offset = None;

        if reserve_scrollbar && content_rect.width > 0 && viewport_height > 0 {
            let track_x = layout.client_x + available_width + SCROLLBAR_GAP;
            let track_rect = Rect::new(
                track_x,
                client_y,
                SCROLLBAR_WIDTH as u32,
                viewport_height as u32,
            );
            if max_scroll > 0 {
                let track_height = viewport_height;
                let ratio = track_height as f32 / content_height.max(1) as f32;
                let mut thumb_height = ((ratio * track_height as f32) + 0.5) as i32;
                thumb_height = clamp_i32(
                    thumb_height,
                    SCROLLBAR_MIN_THUMB.min(track_height),
                    track_height,
                );
                let thumb_travel = (track_height - thumb_height).max(0);
                let thumb_offset = if thumb_travel == 0 || max_scroll == 0 {
                    0
                } else {
                    (((clamped_scroll as f32 / max_scroll as f32) * thumb_travel as f32) + 0.5)
                        as i32
                };
                let thumb_rect = Rect::new(
                    track_rect.x,
                    track_rect.y + thumb_offset,
                    track_rect.width,
                    thumb_height.max(0) as u32,
                );
                scrollbar_track_rect = Some(track_rect);
                scrollbar_thumb_rect = Some(thumb_rect);
                scrollbar_thumb_offset = Some(thumb_offset);
            } else {
                scrollbar_track_rect = Some(track_rect);
                scrollbar_thumb_rect = Some(track_rect);
                scrollbar_thumb_offset = Some(0);
            }
        }

        Self {
            content_rect,
            viewport_height,
            content_height,
            max_scroll,
            scroll_offset: clamped_scroll,
            scrollbar_track_rect,
            scrollbar_thumb_rect,
            scrollbar_thumb_offset,
        }
    }

    pub fn scrollbar_rect(&self) -> Rect {
        self.scrollbar_track_rect.unwrap_or(Rect::new(0, 0, 0, 0))
    }

    pub fn has_scrollbar(&self) -> bool {
        self.max_scroll > 0 && self.scrollbar_track_rect.is_some()
    }

    pub fn clamp_scroll(&self, offset: i32) -> i32 {
        clamp_i32(offset, 0, self.max_scroll)
    }
}

pub fn measure_surface_content_height(surface: &WindowSurface, width: i32) -> i32 {
    if width <= 0 {
        return 0;
    }
    let bitmap_height = surface
        .bitmap
        .as_ref()
        .map(|bmp| bmp.height as i32)
        .unwrap_or(0);
    let text_height = if surface.text.is_empty() {
        0
    } else {
        measure_text_height(&surface.text, width)
    };
    text_height.max(bitmap_height)
}

pub fn measure_text_height(text: &str, width: i32) -> i32 {
    if text.is_empty() || width <= 0 {
        return 0;
    }
    let mut cursor_x = 0;
    let mut cursor_y = FONT_HEIGHT as i32;
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = 0;
            cursor_y += FONT_HEIGHT as i32;
            continue;
        }
        let Some(glyph) = get_glyph(ch) else { continue };
        let gw = glyph.get_width() as i32;
        if cursor_x + gw > width {
            cursor_x = 0;
            cursor_y += FONT_HEIGHT as i32;
        }
        cursor_x += gw;
    }
    cursor_y
}

pub fn point_in_rect(x: i32, y: i32, rect: (i32, i32, i32, i32)) -> bool {
    let (rx, ry, rw, rh) = rect;
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

pub fn rect_contains(rect: &Rect, x: i32, y: i32) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    x >= rect.x && x < rect.x + rect.width as i32 && y >= rect.y && y < rect.y + rect.height as i32
}

pub fn close_button_rect(layout: &WindowLayout) -> (i32, i32, i32, i32) {
    let x = layout.title_x + layout.title_w - CLOSE_BUTTON_SIZE - CLOSE_BUTTON_MARGIN_RIGHT;
    let y = layout.title_y + CLOSE_BUTTON_MARGIN_TOP;
    (x, y, CLOSE_BUTTON_SIZE, CLOSE_BUTTON_SIZE)
}

#[derive(Clone, Copy, Debug)]
pub struct ResizeEdges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

pub fn hit_test_resize(
    win_x: i32,
    win_y: i32,
    win_w: i32,
    win_h: i32,
    cursor_x: i32,
    cursor_y: i32,
) -> Option<ResizeEdges> {
    let left_dist = cursor_x - win_x;
    let right_dist = win_x + win_w - cursor_x;
    let top_dist = cursor_y - win_y;
    let bottom_dist = win_y + win_h - cursor_y;

    let mut edges = ResizeEdges {
        left: false,
        right: false,
        top: false,
        bottom: false,
    };

    let near_left = left_dist >= 0 && left_dist <= RESIZE_MARGIN;
    let near_right = right_dist >= 0 && right_dist <= RESIZE_MARGIN;
    let near_top = top_dist >= 0 && top_dist <= RESIZE_MARGIN;
    let near_bottom = bottom_dist >= 0 && bottom_dist <= RESIZE_MARGIN;

    let corner_hit = |dist_a: i32, dist_b: i32| {
        dist_a >= 0 && dist_b >= 0 && dist_a <= RESIZE_CORNER_SIZE && dist_b <= RESIZE_CORNER_SIZE
    };

    if near_left && corner_hit(left_dist, top_dist) {
        edges.left = true;
        edges.top = true;
    } else if near_right && corner_hit(right_dist, top_dist) {
        edges.right = true;
        edges.top = true;
    } else if near_left && corner_hit(left_dist, bottom_dist) {
        edges.left = true;
        edges.bottom = true;
    } else if near_right && corner_hit(right_dist, bottom_dist) {
        edges.right = true;
        edges.bottom = true;
    } else {
        if near_left {
            edges.left = true;
        } else if near_right {
            edges.right = true;
        }

        if near_top {
            edges.top = true;
        } else if near_bottom {
            edges.bottom = true;
        }
    }

    (edges.left || edges.right || edges.top || edges.bottom).then_some(edges)
}

pub struct DragState {
    pub window_id: Uuid,
    pub kind: DragKind,
}

pub enum DragKind {
    Move {
        offset_x: i32,
        offset_y: i32,
    },
    Resize {
        edges: ResizeEdges,
        start_cursor_x: i32,
        start_cursor_y: i32,
        start_x: i32,
        start_y: i32,
        start_w: i32,
        start_h: i32,
    },
    ScrollThumb {
        track_height: i32,
        thumb_height: i32,
        thumb_offset: i32,
        max_scroll: i32,
        start_cursor_y: i32,
    },
    CloseButton,
}
