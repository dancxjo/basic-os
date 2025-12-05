#![cfg_attr(not(feature = "host"), no_std)]
#![allow(unused)]

extern crate alloc;

use core::prelude::v1::*;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};
use core::convert::TryInto;

use unifont::get_glyph;
use userland::graph;
use userland::graph::GraphPropsRequest;
use userland::widget_abi::WidgetAbi;
use userland::{
    canon, load_thing, println, AbiRequest, AppEvent, FramebufferGeometry, NodePattern, Surface,
    Thingable, Value, WatchId, WatchManager, Window,
};
use uuid::Uuid;
use widget_button::{ButtonWidget, State as ButtonState};

mod framebuffer_backend;
mod layout;

pub use framebuffer_backend::{BitmapFramebufferDevice, BitmapRenderer};
pub use layout::{AlignItems, FlexDirection, JustifyContent, LayoutItem, LayoutSpec};

const FONT_HEIGHT: usize = 16;
const TITLE_BAR_HEIGHT: usize = 32;
const BORDER_OUTER_THICKNESS: i32 = 1;
const BORDER_3D_THICKNESS: i32 = 1;
const BORDER_THICKNESS: i32 = BORDER_OUTER_THICKNESS + BORDER_3D_THICKNESS;
const RESIZE_MARGIN: i32 = 6;
const RESIZE_CORNER_SIZE: i32 = 8;
const MIN_WINDOW_WIDTH: i32 = 140;
const MIN_WINDOW_HEIGHT: i32 = 100;
const CLOSE_BUTTON_SIZE: i32 = 28;
const CLOSE_BUTTON_MARGIN_RIGHT: i32 = 2;
const CLOSE_BUTTON_MARGIN_TOP: i32 = 2;
const TITLE_TEXT_LEFT_PAD: i32 = 8;
const TITLE_TEXT_TOP_OFFSET: i32 = 8;
const TOOLBAR_HEIGHT: i32 = 32;
const TOOLBAR_BUTTON_SIZE: i32 = 24;
const TOOLBAR_BUTTON_SPACING: i32 = 2;
const CURSOR_SIZE: usize = 98;
const SCROLLBAR_WIDTH: i32 = 24; // WCAG 2.2 SC 2.5.8 requires >=24px pointer targets (W3C Oct 2023).
const SCROLLBAR_GAP: i32 = 4;
const SCROLLBAR_MIN_THUMB: i32 = 32; // Keeps the thumb graspable per WCAG 2.5.5 Target Size (Enhanced).
const SCROLL_STEP_LINE: i32 = FONT_HEIGHT as i32;
const SCROLLBAR_TOTAL_RESERVE: i32 = SCROLLBAR_WIDTH + SCROLLBAR_GAP;
const AUTO_TILE_MARGIN: i32 = 8;
const AUTO_TILE_MIN_WINDOWS: usize = 3;
const AUTO_TILE_TOP_OFFSET: i32 = 48;

const ROLE_TOOLBAR: &str = "container.toolbar";
const ROLE_TOOLBAR_BUTTON: &str = "control.toolbar_button";
const ROLE_CONTAINER_VERTICAL: &str = "container.vertical";
const ROLE_EDITOR_ROOT: &str = "container.editor_root";

use core::sync::atomic::{AtomicU64, Ordering};
static UUID_COUNTER: AtomicU64 = AtomicU64::new(0x10000);

fn next_uuid() -> Uuid {
    let id = UUID_COUNTER.fetch_add(1, Ordering::Relaxed);
    Uuid::from_u128(id as u128)
}

const COMPOSITOR_WIDGET: userland::Symbol = canon::canon(b'C', b'M', b'W');

fn create_close_button() -> (Uuid, ButtonState) {
    let widget_id = next_uuid();

    // Create Thing in graph
    let mut props = BTreeMap::new();
    props.insert(canon::KIND, Value::Symbol(COMPOSITOR_WIDGET));
    props.insert(canon::TEXT, Value::Text("✕".to_string()));
    props.insert(canon::TARGET, Value::Text("close_window".to_string()));

    graph::fiat(Some(widget_id), COMPOSITOR_WIDGET, props);

    let state = ButtonState {
        label: "✕".to_string(),
        target: "close_window".to_string(),
        pressed: false,
        hovered: false,
        focused: false,
        icon: None,
        show_label: true,
        bind_node: None,
        bind_index: None,
    };

    (widget_id, state)
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

const THEME: Theme = Theme {
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

// Buttons
const BTN_FACE: Rgba = Rgba::new(0xff, 0xE6, 0xED, 0xF7);
const BTN_BORDER: Rgba = Rgba::new(0xff, 0x5A, 0x6A, 0x8A);
const BTN_GLYPH: Rgba = Rgba::new(0xff, 0xB8, 0x51, 0x51);
// Scrollbar colors maintain >=3:1 contrast per WCAG 2.2 SC 1.4.3 (W3C, Oct 2023).
const SCROLLBAR_TRACK_COLOR: Rgba = Rgba::new(0xff, 0xE2, 0xE6, 0xF0);
const SCROLLBAR_THUMB_COLOR: Rgba = Rgba::new(0xff, 0x7C, 0x8B, 0xAB);
const SCROLLBAR_THUMB_HILIGHT: Rgba = Rgba::new(0xff, 0xF5, 0xF7, 0xFB);
const SCROLLBAR_THUMB_SHADOW: Rgba = Rgba::new(0xff, 0x4A, 0x54, 0x6A);

const COLOR_TEXT: Rgba = THEME.title_text_active;
const COLOR_CURSOR_PRIMARY: Rgba = Rgba::new(0xff, 0xff, 0xff, 0xff);
const COLOR_CURSOR_SHADOW: Rgba = Rgba::new(0x40, 0x00, 0x00, 0x00);
const CLEAR_COLOR: Rgba = Rgba::new(0xff, 0x00, 0x00, 0x00);

struct DragState {
    window_id: Uuid,
    kind: DragKind,
}

#[derive(Clone, Copy, Debug)]
struct ResizeEdges {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorKind {
    Arrow,
    Move,
    ResizeN,
    ResizeS,
    ResizeE,
    ResizeW,
    ResizeNE,
    ResizeNW,
    ResizeSE,
    ResizeSW,
}

enum DragKind {
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

struct WindowLayout {
    title_x: i32,
    title_y: i32,
    title_w: i32,
    title_h: i32,
    client_x: i32,
    client_y: i32,
    client_w: i32,
    client_h: i32,
}

#[derive(Clone, Debug)]
struct ContentMetrics {
    content_rect: Rect,
    viewport_height: i32,
    content_height: i32,
    max_scroll: i32,
    scroll_offset: i32,
    scrollbar_track_rect: Option<Rect>,
    scrollbar_thumb_rect: Option<Rect>,
    scrollbar_thumb_offset: Option<i32>,
}

fn compute_window_layout(win_x: i32, win_y: i32, win_w: i32, win_h: i32) -> Option<WindowLayout> {
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
    fn new(surface: &WindowSurface, layout: &WindowLayout) -> Self {
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

    fn has_scrollbar(&self) -> bool {
        self.max_scroll > 0 && self.scrollbar_track_rect.is_some()
    }

    fn clamp_scroll(&self, offset: i32) -> i32 {
        clamp_i32(offset, 0, self.max_scroll)
    }
}

fn measure_surface_content_height(surface: &WindowSurface, width: i32) -> i32 {
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

fn measure_text_height(text: &str, width: i32) -> i32 {
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

fn point_in_rect(x: i32, y: i32, rect: (i32, i32, i32, i32)) -> bool {
    let (rx, ry, rw, rh) = rect;
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

fn rect_contains(rect: &Rect, x: i32, y: i32) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    x >= rect.x && x < rect.x + rect.width as i32 && y >= rect.y && y < rect.y + rect.height as i32
}

fn close_button_rect(layout: &WindowLayout) -> (i32, i32, i32, i32) {
    let x = layout.title_x + layout.title_w - CLOSE_BUTTON_SIZE - CLOSE_BUTTON_MARGIN_RIGHT;
    let y = layout.title_y + CLOSE_BUTTON_MARGIN_TOP;
    (x, y, CLOSE_BUTTON_SIZE, CLOSE_BUTTON_SIZE)
}

fn hit_test_resize(
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

pub struct FrameInfo {
    pub addr: u64,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u64,
}

pub trait FramebufferDevice<T> {
    fn geometry(&self) -> FramebufferGeometry;
    fn present(&mut self, frame: T);
    fn present_partial(&mut self, frame: T, _dirty_rect: Rect) {
        self.present(frame);
    }
    fn frame_info(&self) -> Option<FrameInfo> {
        None
    }
}

pub trait RendererBackend {
    type Output<'a>
    where
        Self: 'a;
    fn render<'a>(&'a mut self, scene: &Scene) -> Self::Output<'a>;
    fn render_partial<'a>(&'a mut self, scene: &Scene, _dirty_rect: Rect) -> Self::Output<'a> {
        self.render(scene)
    }
}

#[derive(Clone, Debug)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Arc<[u32]>,
}

impl Bitmap {
    pub fn new(width: usize, height: usize, pixels: Vec<u32>) -> Self {
        if pixels.len() != width * height {
            panic!(
                "Bitmap::new: pixels length {} does not match width {} * height {}",
                pixels.len(),
                width,
                height
            );
        }
        Self {
            width,
            height,
            pixels: pixels.into(),
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Option<u32> {
        if x >= self.width || y >= self.height {
            None
        } else {
            Some(self.pixels[y * self.width + x])
        }
    }

    pub fn sample(&self, x: usize, y: usize) -> u32 {
        if self.width == 0 || self.height == 0 {
            return 0;
        }
        let sx = x % self.width;
        let sy = y % self.height;
        self.pixels[sy * self.width + sx]
    }
}

#[derive(Clone, Debug, Default)]
pub struct Caret {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub visible: bool,
}

#[derive(Clone)]
struct WindowSurface {
    window: Window,
    surface_id: Option<Uuid>,
    text: String,
    bitmap: Option<Arc<Bitmap>>,
    scroll_y: i32,
    scrollbar_widget_id: Option<Uuid>,
    caret: Caret,
    close_button_id: Uuid,
    close_button_state: ButtonState,
    close_button_bitmap: Option<Arc<Bitmap>>,
}

#[derive(Clone)]
struct CursorIcon {
    bitmap: Arc<Bitmap>,
    hotspot: (i32, i32),
}

struct CursorSprites {
    arrow: CursorIcon,
    move_icon: CursorIcon,
    resize_ns: CursorIcon,
    resize_ew: CursorIcon,
    resize_ne_sw: CursorIcon,
    resize_nw_se: CursorIcon,
}

impl CursorSprites {
    fn for_kind(&self, kind: CursorKind) -> &CursorIcon {
        match kind {
            CursorKind::Arrow => &self.arrow,
            CursorKind::Move => &self.move_icon,
            CursorKind::ResizeN | CursorKind::ResizeS => &self.resize_ns,
            CursorKind::ResizeE | CursorKind::ResizeW => &self.resize_ew,
            CursorKind::ResizeNE | CursorKind::ResizeSW => &self.resize_ne_sw,
            CursorKind::ResizeNW | CursorKind::ResizeSE => &self.resize_nw_se,
        }
    }
}

struct CursorState {
    x: i32,
    y: i32,
    buttons: u8,
    visible: bool,
    kind: CursorKind,
}

impl CursorState {
    fn new(width: usize, height: usize) -> Self {
        Self {
            x: (width / 2) as i32,
            y: (height / 2) as i32,
            buttons: 0,
            visible: true,
            kind: CursorKind::Arrow,
        }
    }

    fn update(&mut self, dx: i64, dy: i64, buttons: u8, width: usize, height: usize) {
        self.x = clamp_i32(self.x + dx as i32, 0, width.saturating_sub(1) as i32);
        self.y = clamp_i32(self.y + dy as i32, 0, height.saturating_sub(1) as i32);
        self.buttons = buttons;
    }

    fn set_from_graph(
        &mut self,
        x: Option<i64>,
        y: Option<i64>,
        visible: Option<bool>,
        width: usize,
        height: usize,
    ) {
        if let Some(nx) = x {
            self.x = clamp_i32(nx as i32, 0, width.saturating_sub(1) as i32);
        }
        if let Some(ny) = y {
            self.y = clamp_i32(ny as i32, 0, height.saturating_sub(1) as i32);
        }
        if let Some(vis) = visible {
            self.visible = vis;
        }
    }

    fn set_kind(&mut self, kind: CursorKind) {
        self.kind = kind;
    }
}

#[derive(Clone, Debug)]
pub struct ModeSlot {
    pub mode_index: u8,
    pub root_window: Option<Uuid>,
    pub windows: Vec<Uuid>,
}

impl ModeSlot {
    pub fn new(index: u8) -> Self {
        Self {
            mode_index: index,
            root_window: None,
            windows: Vec::new(),
        }
    }
}

pub struct Compositor<F, R> {
    frame_no: u64,
    fb_device: F,
    renderer: R,
    windows: BTreeMap<Uuid, WindowSurface>,
    watch_surfaces: Option<WatchId>,
    watch_windows: Option<WatchId>,
    watch_mouse: Option<WatchId>,
    watch_keyboard: Option<WatchId>,
    watch_cursor: Option<WatchId>,
    watch_fb: Option<WatchId>,
    watch_widgets: Option<WatchId>,
    fb_id: Option<Uuid>,
    fb_dirty: bool,
    content_dirty: bool,
    auto_layout_done: bool,
    cursor: CursorState,
    cursor_sprites: CursorSprites,
    active_window: Option<Uuid>,
    active_mode: usize,
    modes: [ModeSlot; 12],
    saved_sky_geometry: BTreeMap<Uuid, Rect>,
    theme: Theme,
    background: Arc<Bitmap>,
    drag_state: Option<DragState>,
    alt_down: bool,
    shift_down: bool,
    state_node: Uuid,
    widgets: BTreeMap<Uuid, userland::ui_graph::Widget>,
    active_widget: Option<Uuid>,
    debug_layout_mode: bool,
    debug_overlay_mode: bool,
    cursor_prev_rect: Option<Rect>,
}

impl<F, R> Compositor<F, R>
where
    R: RendererBackend,
    F: for<'a> FramebufferDevice<R::Output<'a>>,
{
    pub fn new(fb_device: F, renderer: R) -> Self {
        let geo = fb_device.geometry();
        let (width, height) = (geo.width as usize, geo.height as usize);
        let background = load_background();
        let theme = THEME;
        let cursor_sprites = build_cursor_sprites();

        let state_node = userland::simple_uuid(b"CompositorState");
        let mut fields = userland::map();
        fields.insert(canon::NAME, Value::Text("CompositorState".into()));
        userland::fiat(Some(state_node), canon::COMPOSITOR, fields);

        let modes = core::array::from_fn(|i| ModeSlot::new(i as u8));

        Self {
            frame_no: 0,
            fb_device,
            renderer,
            windows: BTreeMap::new(),
            watch_surfaces: None,
            watch_windows: None,
            watch_mouse: None,
            watch_keyboard: None,
            watch_cursor: None,
            watch_fb: None,
            watch_widgets: None,
            fb_id: None,
            fb_dirty: false,
            content_dirty: true,
            auto_layout_done: false,
            cursor: CursorState::new(width, height),
            cursor_sprites,
            active_window: None,
            active_mode: 0,
            modes,
            saved_sky_geometry: BTreeMap::new(),
            theme,
            background,
            drag_state: None,
            alt_down: false,
            shift_down: false,
            state_node,
            widgets: BTreeMap::new(),
            active_widget: None,
            debug_layout_mode: true,
            debug_overlay_mode: false,
            cursor_prev_rect: None,
        }
    }

    pub fn fb_device(&self) -> &F {
        &self.fb_device
    }

    pub fn fb_device_mut(&mut self) -> &mut F {
        &mut self.fb_device
    }

    pub fn renderer(&self) -> &R {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut R {
        &mut self.renderer
    }

    pub fn is_fb_dirty(&self) -> bool {
        self.fb_dirty
    }

    pub fn clear_fb_dirty(&mut self) {
        self.fb_dirty = false;
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.cursor.x = clamp_i32(self.cursor.x, 0, width.saturating_sub(1) as i32);
        self.cursor.y = clamp_i32(self.cursor.y, 0, height.saturating_sub(1) as i32);
    }

    fn update_graph_state(&self) {
        let mut fields = userland::map();

        // Window order (front to back)
        let mut order_ids = Vec::new();
        for id in self.ordered_window_ids().iter().rev() {
            order_ids.push(Value::Uuid(*id));
        }
        fields.insert(canon::ABOVE, Value::List(order_ids));

        // Active window
        if let Some(active) = self.active_window {
            fields.insert(canon::ACTIVE_WINDOW, Value::Uuid(active));
        } else {
            fields.insert(canon::ACTIVE_WINDOW, Value::Null);
        }

        userland::fiat(Some(self.state_node), canon::COMPOSITOR, fields);
    }

    pub fn init_with_watches(
        watch_manager: &mut WatchManager,
        app_id: usize,
        fb_device: F,
        renderer: R,
    ) -> Self {
        let mut surface_pattern = NodePattern::default();
        surface_pattern.labels.push(canon::SURFACE);
        surface_pattern
            .props
            .insert(canon::DIRTY, Value::Bool(true));

        let mut surface_discovery = NodePattern::default();
        surface_discovery.labels.push(canon::SURFACE);

        let mut window_pattern = NodePattern::default();
        window_pattern.labels.push(canon::WINDOW);

        let mut cursor_pattern = NodePattern::default();
        cursor_pattern.labels.push(canon::CURSOR);

        let mut fb_pattern = NodePattern::default();
        fb_pattern.labels.push(canon::DISPLAY_FRAMEBUFFER);

        let mut mouse_pattern = NodePattern::default();
        mouse_pattern.labels.push(canon::INPUT_EVENT);

        let mut keyboard_pattern = NodePattern::default();
        keyboard_pattern.labels.push(canon::KEY_EVENT);

        let mut widget_pattern = NodePattern::default();
        widget_pattern.labels.push(canon::WIDGET);

        let surface_watch = watch_manager.register_pattern(app_id, surface_pattern.clone());
        let window_watch = watch_manager.register_pattern(app_id, window_pattern.clone());
        let cursor_watch = watch_manager.register_pattern(app_id, cursor_pattern.clone());
        let fb_watch = watch_manager.register_pattern(app_id, fb_pattern.clone());
        let mouse_watch = watch_manager.register_pattern(app_id, mouse_pattern);
        let keyboard_watch = watch_manager.register_pattern(app_id, keyboard_pattern);
        let widget_watch = watch_manager.register_pattern(app_id, widget_pattern.clone());

        let mut comp = Self::new(fb_device, renderer);
        comp.watch_surfaces = Some(surface_watch);
        comp.watch_windows = Some(window_watch);
        comp.watch_mouse = Some(mouse_watch);
        comp.watch_keyboard = Some(keyboard_watch);
        comp.watch_cursor = Some(cursor_watch);
        comp.watch_fb = Some(fb_watch);
        comp.watch_widgets = Some(widget_watch);

        for thing in userland::graph::get_nodes(window_pattern) {
            if let Some(window) = Window::load(&thing) {
                comp.ingest_window(window);
            }
        }

        for thing in userland::graph::get_nodes(surface_discovery) {
            comp.ingest_surface(&thing);
        }

        for thing in userland::graph::get_nodes(widget_pattern) {
            comp.ingest_widget(&thing);
        }

        if let Some(cursor_node) = userland::graph::get_nodes(cursor_pattern)
            .into_iter()
            .next()
        {
            comp.ingest_cursor(&cursor_node);
        }

        comp
    }

    pub fn on_event(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::Thing { watch, thing } => {
                if Some(*watch) == self.watch_surfaces {
                    self.ingest_surface(thing);
                    self.content_dirty = true;
                } else if Some(*watch) == self.watch_mouse {
                    if thing.kind == canon::INPUT_EVENT {
                        self.ingest_input_event(thing);
                    }
                } else if Some(*watch) == self.watch_keyboard {
                    if thing.kind == canon::KEY_EVENT {
                        self.ingest_key_event(thing);
                    }
                } else if Some(*watch) == self.watch_windows {
                    if let Some(window) = Window::load(thing) {
                        self.ingest_window(window);
                        self.content_dirty = true;
                    }
                } else if Some(*watch) == self.watch_cursor {
                    self.ingest_cursor(thing);
                    // Cursor ingest might change cursor appearance, but position is handled by mouse events?
                    // ingest_cursor updates cursor state?
                    // Let's check ingest_cursor.
                } else if Some(*watch) == self.watch_fb {
                    if thing.kind == canon::DISPLAY_FRAMEBUFFER {
                        self.fb_id = Some(thing.id);
                        self.fb_dirty = true;
                    }
                } else if Some(*watch) == self.watch_widgets {
                    self.ingest_widget(thing);
                    self.content_dirty = true;
                }
            }
            AppEvent::Edge { .. } => {
                // No edge handling needed for now
            }
        }
    }

    fn ensure_scrollbar(&mut self, window_id: Uuid, metrics: &ContentMetrics) {
        let track_rect = metrics.scrollbar_track_rect;

        let widget_id = if let Some(w) = self.windows.get(&window_id) {
            w.scrollbar_widget_id
        } else {
            return;
        };

        if let Some(rect) = track_rect {
            if let Some(id) = widget_id {
                let mut updates = BTreeMap::new();
                updates.insert(canon::X, Value::U64(rect.x as u64));
                updates.insert(canon::Y, Value::U64(rect.y as u64));
                updates.insert(canon::WIDTH, Value::U64(rect.width as u64));
                updates.insert(canon::HEIGHT, Value::U64(rect.height as u64));

                updates.insert(
                    canon::VIEWPORT_HEIGHT,
                    Value::I64(metrics.viewport_height as i64),
                );
                updates.insert(
                    canon::CONTENT_HEIGHT,
                    Value::I64(metrics.content_height as i64),
                );
                updates.insert(canon::SCROLL_Y, Value::I64(metrics.scroll_offset as i64));

                userland::graph::fiat(Some(id), canon::WIDGET, updates);
            } else {
                let mut fields = BTreeMap::new();
                fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
                fields.insert(canon::X, Value::U64(rect.x as u64));
                fields.insert(canon::Y, Value::U64(rect.y as u64));
                fields.insert(canon::WIDTH, Value::U64(rect.width as u64));
                fields.insert(canon::HEIGHT, Value::U64(rect.height as u64));

                fields.insert(
                    canon::cc('W', 'K'),
                    Value::Text(String::from("scrollbar_thumb")),
                );
                fields.insert(canon::PARENT, Value::Uuid(window_id));

                fields.insert(
                    canon::VIEWPORT_HEIGHT,
                    Value::I64(metrics.viewport_height as i64),
                );
                fields.insert(
                    canon::CONTENT_HEIGHT,
                    Value::I64(metrics.content_height as i64),
                );
                fields.insert(canon::SCROLL_Y, Value::I64(metrics.scroll_offset as i64));

                let id = userland::graph::fiat(None, canon::WIDGET, fields);

                if let Some(w) = self.windows.get_mut(&window_id) {
                    w.scrollbar_widget_id = Some(id);
                }
            }
        } else {
            if let Some(id) = widget_id {
                let mut updates = BTreeMap::new();
                updates.insert(canon::WIDTH, Value::U64(0));
                updates.insert(canon::HEIGHT, Value::U64(0));
                userland::graph::fiat(Some(id), canon::WIDGET, updates);
            }
        }
    }

    fn update_scrollbars(&mut self) {
        let ids: Vec<Uuid> = self.windows.keys().cloned().collect();
        for id in ids {
            let metrics = {
                if let Some(s) = self.windows.get(&id) {
                    if let Some(layout) = compute_window_layout(
                        s.window.x as i32,
                        s.window.y as i32,
                        s.window.width as i32,
                        s.window.height as i32,
                    ) {
                        Some(ContentMetrics::new(s, &layout))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(m) = metrics {
                self.ensure_scrollbar(id, &m);
            }
        }
    }

    fn get_cursor_rect(&self) -> Option<Rect> {
        if !self.cursor.visible {
            return None;
        }
        let icon = self.cursor_sprites.for_kind(self.cursor.kind);
        let w = icon.bitmap.width as u32;
        let h = icon.bitmap.height as u32;
        let hotspot = icon.hotspot;

        let x = self.cursor.x - hotspot.0;
        let y = self.cursor.y - hotspot.1;

        Some(Rect::new(x, y, w, h))
    }

    fn draw_root_window(
        &self,
        scene: &mut Scene,
        surface: &WindowSurface,
        fb_width: usize,
        fb_height: usize,
    ) {
        let x = 0;
        let y = 0;
        let w = fb_width;
        let h = fb_height;

        scene.push(SceneItem::ClipPush {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
        });

        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
            color: self.theme.client_bg,
        });

        if let Some(bmp) = &surface.bitmap {
            scene.push(SceneItem::BlitImage {
                rect: Rect::new(x as i32, y as i32, bmp.width as u32, bmp.height as u32),
                image: bmp.clone(),
                repeat: false,
                offset: (0, 0),
            });
        } else if !surface.text.is_empty() {
            scene.push(SceneItem::DrawTextBlock {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                text: surface.text.clone(),
                color: COLOR_TEXT,
                scroll_offset: surface.scroll_y,
            });
        }

        let has_widgets = self
            .widgets
            .values()
            .any(|w| w.parent == Some(surface.window.id));

        if has_widgets {
            let layout = WindowLayout {
                title_x: 0,
                title_y: 0,
                title_w: 0,
                title_h: 0,
                client_x: x as i32,
                client_y: y as i32,
                client_w: w as i32,
                client_h: h as i32,
            };
            self.draw_widgets(scene, surface.window.id, &layout, layout.client_w);
        }

        scene.push(SceneItem::ClipPop);
    }

    fn draw_mode(&mut self, scene: &mut Scene, width: usize, height: usize) {
        let mode = &self.modes[self.active_mode];

        if let Some(root_id) = mode.root_window {
            if let Some(surface) = self.windows.get(&root_id).cloned() {
                self.draw_root_window(scene, &surface, width, height);
            }
        } else {
            self.draw_background(scene, width, height);
        }

        for id in &mode.windows {
            if let Some(surface) = self.windows.get(id).cloned() {
                self.draw_window(scene, &surface, width, height);
            }
        }
    }

    pub fn tick(&mut self) {
        let geo = self.fb_device.geometry();
        let (width, height) = (geo.width as usize, geo.height as usize);
        if width == 0 || height == 0 {
            return;
        }

        self.ensure_active_window();
        self.update_cursor_kind();
        self.update_scrollbars();

        let cursor_new_rect = self.get_cursor_rect();
        let cursor_moved = cursor_new_rect != self.cursor_prev_rect;

        if !self.fb_dirty && !self.content_dirty && !cursor_moved {
            return;
        }

        let mut scene = Scene::new(width as u32, height as u32);
        scene.push(SceneItem::Clear { color: CLEAR_COLOR });

        self.draw_mode(&mut scene, width, height);

        self.draw_cursor(&mut scene, width, height);

        if self.fb_dirty || self.content_dirty {
            let frame = self.renderer.render(&scene);
            self.fb_device.present(frame);
            self.content_dirty = false;
        } else if cursor_moved {
            let dirty_rect = if let Some(prev) = self.cursor_prev_rect {
                if let Some(curr) = cursor_new_rect {
                    prev.union(curr)
                } else {
                    prev
                }
            } else {
                cursor_new_rect.unwrap_or(Rect::new(0, 0, 0, 0))
            };

            let frame = self.renderer.render_partial(&scene, dirty_rect);
            self.fb_device.present_partial(frame, dirty_rect);
        }

        self.cursor_prev_rect = cursor_new_rect;
        self.publish_frame_info();
        self.frame_no = self.frame_no.wrapping_add(1);
    }

    fn publish_frame_info(&mut self) {
        if let Some(fb_id) = self.fb_id {
            if let Some(info) = self.fb_device.frame_info() {
                let mut fields = BTreeMap::new();
                fields.insert(canon::KIND, Value::Symbol(canon::DISPLAY_FRAME));
                fields.insert(canon::SEQ, Value::U64(self.frame_no));
                fields.insert(canon::ADDR, Value::U64(info.addr));
                fields.insert(canon::WIDTH, Value::U64(info.width));
                fields.insert(canon::HEIGHT, Value::U64(info.height));
                fields.insert(canon::PITCH, Value::U64(info.pitch));
                fields.insert(canon::BPP, Value::U64(info.bpp));

                let frame_id = userland::fiat(None, canon::DISPLAY_FRAME, fields);
                userland::that(fb_id, canon::CURRENT_FRAME, frame_id, 0);
            }
        }
    }

    fn find_window_at(&self, x: i32, y: i32) -> Option<(Uuid, i32, i32)> {
        for win_id in self.ordered_window_ids().into_iter().rev() {
            let surface = self.windows.get(&win_id)?;
            if !surface.window.visible {
                continue;
            }
            let wx = surface.window.x as i32;
            let wy = surface.window.y as i32;
            let w = surface.window.width as i32;
            let h = surface.window.height as i32;

            if x >= wx && x < wx + w && y >= wy && y < wy + h {
                return Some((win_id, wx, wy));
            }
        }
        None
    }

    fn content_metrics_for_window(
        &self,
        window_id: Uuid,
    ) -> Option<(WindowLayout, ContentMetrics)> {
        let surface = self.windows.get(&window_id)?;
        let layout = compute_window_layout(
            surface.window.x as i32,
            surface.window.y as i32,
            surface.window.width as i32,
            surface.window.height as i32,
        )?;
        let metrics = ContentMetrics::new(surface, &layout);
        Some((layout, metrics))
    }

    fn clamp_scroll_for(&mut self, window_id: Uuid) {
        if let Some((_, metrics)) = self.content_metrics_for_window(window_id) {
            if let Some(surface) = self.windows.get_mut(&window_id) {
                if surface.scroll_y != metrics.scroll_offset {
                    surface.scroll_y = metrics.scroll_offset;
                }
            }
        } else if let Some(surface) = self.windows.get_mut(&window_id) {
            surface.scroll_y = 0;
        }
    }

    fn apply_window_rect_hint(&mut self, window_id: Uuid) {
        if let Some(surface) = self.windows.get_mut(&window_id) {
            if let Some(rect) = surface.window.window_rect {
                surface.caret.x = rect.x as i32;
                surface.caret.y = rect.y as i32;
                surface.caret.width = 2;
                surface.caret.height = rect.height as i32;
                surface.caret.visible = rect.visible;
            } else {
                surface.caret.visible = false;
            }
        }

        let Some(rect) = self
            .windows
            .get(&window_id)
            .and_then(|surface| surface.window.window_rect)
        else {
            return;
        };
        let Some((_, metrics)) = self.content_metrics_for_window(window_id) else {
            return;
        };
        if metrics.viewport_height <= 0 {
            return;
        }

        let current_scroll = self
            .windows
            .get(&window_id)
            .map(|surface| surface.scroll_y)
            .unwrap_or(0);
        let rect_top = clamp_i32(
            rect.y.clamp(0, i64::from(i32::MAX)).try_into().unwrap_or(0),
            0,
            i32::MAX,
        );
        let raw_height = rect.height.max(0);
        let rect_height = clamp_i32(
            raw_height
                .min(i64::from(i32::MAX))
                .try_into()
                .unwrap_or(FONT_HEIGHT as i32),
            FONT_HEIGHT as i32,
            i32::MAX,
        );
        let rect_bottom = rect_top.saturating_add(rect_height);
        let viewport_bottom = current_scroll + metrics.viewport_height;

        let mut desired_scroll = current_scroll;
        if rect_top < current_scroll {
            desired_scroll = rect_top;
        } else if rect_bottom > viewport_bottom {
            desired_scroll = rect_bottom - metrics.viewport_height;
        } else {
            return;
        }

        let clamped = metrics.clamp_scroll(desired_scroll);
        if let Some(surface) = self.windows.get_mut(&window_id) {
            if surface.scroll_y != clamped {
                surface.scroll_y = clamped;
            }
        }
    }

    fn set_scroll_offset(&mut self, window_id: Uuid, new_offset: i32) -> bool {
        if let Some((_, metrics)) = self.content_metrics_for_window(window_id) {
            if metrics.max_scroll <= 0 {
                if let Some(surface) = self.windows.get_mut(&window_id) {
                    surface.scroll_y = 0;
                }
                return false;
            }
            let clamped = metrics.clamp_scroll(new_offset);
            if let Some(surface) = self.windows.get_mut(&window_id) {
                if surface.scroll_y != clamped {
                    surface.scroll_y = clamped;
                    return true;
                }
            }
        }
        false
    }

    fn scroll_window_by(&mut self, window_id: Uuid, delta: i32) -> bool {
        if delta == 0 {
            return false;
        }
        if let Some(surface) = self.windows.get(&window_id) {
            let new_offset = surface.scroll_y.saturating_add(delta);
            return self.set_scroll_offset(window_id, new_offset);
        }
        false
    }

    fn scroll_window_to_start(&mut self, window_id: Uuid) -> bool {
        self.set_scroll_offset(window_id, 0)
    }

    fn scroll_window_to_end(&mut self, window_id: Uuid) -> bool {
        if let Some((_, metrics)) = self.content_metrics_for_window(window_id) {
            if metrics.max_scroll > 0 {
                return self.set_scroll_offset(window_id, metrics.max_scroll);
            }
        }
        false
    }

    // Keyboard navigation for scrolling keeps UIs operable per WCAG 2.2 SC 2.1.1 (Keyboard).
    fn handle_scroll_key(&mut self, key: canon::Symbol) -> bool {
        let Some(active) = self.active_window else {
            return false;
        };
        let Some((_, metrics)) = self.content_metrics_for_window(active) else {
            return false;
        };
        if metrics.max_scroll <= 0 {
            return false;
        }
        let page = metrics.viewport_height.max(SCROLL_STEP_LINE);
        let delta = match key {
            k if k == canon::cc('A', 'U') => Some(-SCROLL_STEP_LINE),
            k if k == canon::cc('A', 'D') => Some(SCROLL_STEP_LINE),
            k if k == canon::cc('P', 'U') => Some(-page),
            k if k == canon::cc('P', 'D') => Some(page),
            _ => None,
        };
        if let Some(delta) = delta {
            return self.scroll_window_by(active, delta);
        }
        if key == canon::cc('H', 'M') {
            return self.scroll_window_to_start(active);
        }
        if key == canon::cc('E', 'D') {
            return self.scroll_window_to_end(active);
        }
        false
    }

    fn get_widget_height(&self, widget: &userland::ui_graph::Widget, w: i32, h: i32) -> i32 {
        if !widget.visible {
            return 0;
        }
        if widget.role == ROLE_TOOLBAR {
            return TOOLBAR_HEIGHT;
        } else if widget.role == ROLE_TOOLBAR_BUTTON || widget.role == "toolbar_button" {
            return widget.height.map(|v| v as i32).unwrap_or(32);
        } else if widget.role == ROLE_CONTAINER_VERTICAL || widget.role == "window_root" {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();
            let mut height = 0;
            let mut remaining_h = h;
            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let ch = self.get_widget_height(child, w, remaining_h);
                    height += ch;
                    remaining_h -= ch;
                }
            }
            return height;
        } else if widget.role == ROLE_EDITOR_ROOT {
            return h;
        }
        0
    }

    fn hit_test_widgets(
        &self,
        window_id: Uuid,
        layout: &WindowLayout,
        metrics: &ContentMetrics,
        mx: i32,
        my: i32,
    ) -> Option<Uuid> {
        let root_widgets: Vec<Uuid> = self
            .widgets
            .values()
            .filter(|w| w.parent == Some(window_id))
            .map(|w| w.id)
            .collect();

        // Check absolute positioned widgets first (like scrollbars)
        for widget_id in &root_widgets {
            if let Some(widget) = self.widgets.get(widget_id) {
                if let (Some(x), Some(y), Some(w), Some(h)) =
                    (widget.x, widget.y, widget.width, widget.height)
                {
                    let x = x as i32;
                    let y = y as i32;
                    let w = w as i32;
                    let h = h as i32;
                    if mx >= x && mx < x + w && my >= y && my < y + h {
                        return Some(*widget_id);
                    }
                }
            }
        }

        let y_offset = layout.client_y;
        let x_offset = layout.client_x;
        let widget_width = if metrics.content_rect.width > 0 {
            metrics.content_rect.width as i32
        } else {
            layout.client_w
        };
        let width = widget_width.max(0);
        let height = layout.client_h;

        for widget_id in root_widgets {
            if let Some(widget) = self.widgets.get(&widget_id) {
                if widget.x.is_some() && widget.y.is_some() {
                    continue;
                }

                if let Some(hit) = self
                    .hit_test_widget_recursive(widget, x_offset, y_offset, width, height, mx, my)
                {
                    return Some(hit);
                }
            }
        }
        None
    }

    fn hit_test_widget_recursive(
        &self,
        widget: &userland::ui_graph::Widget,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        mx: i32,
        my: i32,
    ) -> Option<Uuid> {
        if !widget.visible {
            return None;
        }

        let height = self.get_widget_height(widget, w, h);

        if mx < x || mx >= x + w || my < y || my >= y + height {
            return None;
        }

        if widget.role == ROLE_TOOLBAR {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_x = x;
            let toolbar_end = x + w;
            let drawn_height = TOOLBAR_HEIGHT;
            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let btn_w = TOOLBAR_BUTTON_SIZE;
                    let btn_h = TOOLBAR_BUTTON_SIZE;
                    let btn_y = y + (drawn_height - btn_h) / 2;

                    if child_x + btn_w > toolbar_end {
                        break;
                    }

                    if mx >= child_x && mx < child_x + btn_w && my >= btn_y && my < btn_y + btn_h {
                        return Some(child.id);
                    }
                    child_x += btn_w + TOOLBAR_BUTTON_SPACING;
                }
            }
            return Some(widget.id);
        } else if widget.role == ROLE_CONTAINER_VERTICAL || widget.role == "window_root" {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y;
            let mut remaining_h = h;

            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let child_h = self.get_widget_height(child, w, remaining_h);
                    if let Some(hit) =
                        self.hit_test_widget_recursive(child, x, child_y, w, remaining_h, mx, my)
                    {
                        return Some(hit);
                    }
                    child_y += child_h;
                    remaining_h -= child_h;
                }
            }
            return Some(widget.id);
        } else if widget.role == ROLE_EDITOR_ROOT {
            return Some(widget.id);
        } else if widget.role == ROLE_TOOLBAR_BUTTON || widget.role == "toolbar_button" {
            return Some(widget.id);
        }

        if widget.role == ROLE_TOOLBAR_BUTTON {
            return Some(widget.id);
        }

        None
    }

    fn draw_widgets(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        layout: &WindowLayout,
        widget_area_width: i32,
    ) {
        let root_widgets: Vec<Uuid> = self
            .widgets
            .values()
            .filter(|w| w.parent == Some(window_id))
            .map(|w| w.id)
            .collect();

        if root_widgets.is_empty() {
            return;
        }

        let mut relative_widgets: Vec<Uuid> = Vec::new();
        let mut overlay_widgets: Vec<Uuid> = Vec::new();

        for widget_id in &root_widgets {
            if let Some(widget) = self.widgets.get(widget_id) {
                if widget.x.is_some() && widget.y.is_some() {
                    overlay_widgets.push(*widget_id);
                } else {
                    relative_widgets.push(*widget_id);
                }
            }
        }

        let x_offset = layout.client_x;
        let width = widget_area_width.max(0);
        let y_offset = layout.client_y;
        let height = layout.client_h.max(0);

        // Determine layout spec from window properties
        let (gap, spec) = if let Some(surface) = self.windows.get(&window_id) {
            let w = &surface.window;
            let gap = w.gap.unwrap_or(0);
            let (direction, justify, align) = if let Some(dir) = w.flex_direction {
                (
                    dir,
                    w.justify_content.unwrap_or_default(),
                    w.align_items.unwrap_or(AlignItems::Start),
                )
            } else {
                (
                    FlexDirection::Column,
                    JustifyContent::Start,
                    AlignItems::Stretch,
                )
            };
            let spec = LayoutSpec::Flex {
                direction,
                justify,
                align,
            };
            (gap, spec)
        } else {
            (
                0,
                LayoutSpec::Flex {
                    direction: FlexDirection::Column,
                    justify: JustifyContent::Start,
                    align: AlignItems::Stretch,
                },
            )
        };

        // Use layout engine for relative widgets
        if !relative_widgets.is_empty() {
            self.layout_and_draw_children(
                scene,
                window_id,
                Rect::new(
                    x_offset as i32,
                    y_offset as i32,
                    width as u32,
                    height as u32,
                ),
                &relative_widgets,
                spec,
                gap,
            );
        }

        for widget_id in overlay_widgets {
            if let Some(widget) = self.widgets.get(&widget_id) {
                if let (Some(x), Some(y), Some(w), Some(h)) =
                    (widget.x, widget.y, widget.width, widget.height)
                {
                    self.draw_widget_recursive(
                        scene, window_id, widget, x as i32, y as i32, w as i32, h as i32,
                    );
                }
            }
        }
    }

    fn layout_and_draw_children(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        container_rect: Rect,
        children_ids: &[Uuid],
        spec: LayoutSpec,
        gap: i32,
    ) {
        let items: Vec<LayoutItem> = children_ids
            .iter()
            .filter_map(|id| {
                self.widgets.get(id).map(|w| LayoutItem {
                    id: *id,
                    min_width: w.width.unwrap_or(0) as u32,
                    min_height: w.height.unwrap_or(30) as u32, // Default height 30 if unknown
                    flex_grow: w.flex_grow.unwrap_or(0.0),
                    flex_shrink: w.flex_shrink.unwrap_or(1.0),
                    ..Default::default()
                })
            })
            .collect();

        let rects = layout::layout(container_rect, spec, &items, gap);

        for (child_id, rect) in &rects {
            if let Some(child) = self.widgets.get(child_id) {
                if child.kind.is_none() {
                    continue;
                }
                let width_changed = child.width.map(|w| w as u32 != rect.width).unwrap_or(true);
                let height_changed = child
                    .height
                    .map(|h| h as u32 != rect.height)
                    .unwrap_or(true);

                if width_changed || height_changed {
                    let mut updates = graph::map();
                    if width_changed {
                        updates.insert(canon::WIDTH, Value::U64(rect.width as u64));
                    }
                    if height_changed {
                        updates.insert(canon::HEIGHT, Value::U64(rect.height as u64));
                    }
                    graph::fiat(Some(*child_id), canon::WIDGET, updates);
                }
            }
        }

        for (child_id, rect) in rects {
            if let Some(child) = self.widgets.get(&child_id) {
                self.draw_widget_recursive(
                    scene,
                    window_id,
                    child,
                    rect.x,
                    rect.y,
                    rect.width as i32,
                    rect.height as i32,
                );
            }
        }
    }

    fn draw_widget_recursive(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        widget: &userland::ui_graph::Widget,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> i32 {
        if !widget.visible {
            return 0;
        }

        if let Some(bytes) = &widget.bitmap {
            if let (Some(bw), Some(bh)) = (widget.width, widget.height) {
                let expected_len = (bw * bh) as usize;
                let mut pixels = Vec::with_capacity(expected_len);
                for chunk in bytes.chunks(4) {
                    if pixels.len() >= expected_len {
                        break;
                    }
                    if chunk.len() == 4 {
                        let b = chunk[0] as u32; // Blue
                        let g = chunk[1] as u32; // Green
                        let r = chunk[2] as u32; // Red
                        let a = chunk[3] as u32; // Alpha
                                                 // ARGB
                        let val = (a << 24) | (r << 16) | (g << 8) | b;
                        pixels.push(val);
                    } else {
                        pixels.push(0);
                    }
                }

                // Pad with transparent pixels if the source data is smaller than the declared dimensions
                while pixels.len() < expected_len {
                    pixels.push(0);
                }

                let bmp = Arc::new(Bitmap::new(bw as usize, bh as usize, pixels));

                scene.push(SceneItem::BlitImage {
                    rect: Rect::new(x, y, w as u32, h as u32),
                    image: bmp,
                    repeat: false,
                    offset: (0, 0),
                });

                return h;
            }
        }

        let mut drawn_height = 0;

        if widget.role == ROLE_TOOLBAR || widget.role == "toolbar" {
            drawn_height = TOOLBAR_HEIGHT;
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y, w as u32, drawn_height as u32),
                color: THEME.client_bg,
            });
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y + drawn_height - 1, w as u32, 1),
                color: THEME.frame_shadow,
            });

            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_x = x;
            let toolbar_end = x + w;
            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let btn_w = TOOLBAR_BUTTON_SIZE;
                    let btn_h = TOOLBAR_BUTTON_SIZE;
                    let btn_y = y + (drawn_height - btn_h) / 2;

                    if child_x + btn_w > toolbar_end {
                        break;
                    }

                    self.draw_toolbar_button(scene, child, child_x, btn_y, btn_w, btn_h);
                    child_x += btn_w + TOOLBAR_BUTTON_SPACING;
                }
            }
        } else if widget.role == ROLE_CONTAINER_VERTICAL || widget.role == "window_root" {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let (direction, justify, align) = if let Some(dir) = widget.flex_direction {
                (
                    dir,
                    widget.justify_content.unwrap_or_default(),
                    widget.align_items.unwrap_or(AlignItems::Start),
                )
            } else {
                (
                    FlexDirection::Column,
                    JustifyContent::Start,
                    AlignItems::Stretch,
                )
            };

            let spec = LayoutSpec::Flex {
                direction,
                justify,
                align,
            };

            let gap = widget.gap.unwrap_or(0);
            self.layout_and_draw_children(
                scene,
                window_id,
                Rect::new(x, y, w as u32, h as u32),
                &children,
                spec,
                gap,
            );
            drawn_height = h;
        } else if widget.role == ROLE_EDITOR_ROOT {
            drawn_height = h;
            self.draw_surface_content(scene, window_id, x, y, w, h);
        } else if widget.role == "button" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(30);
            self.draw_toolbar_button(scene, widget, x, y, w, drawn_height);
            if let Some(label) = &widget.label {
                scene.push(SceneItem::DrawTextBlock {
                    rect: Rect::new(x + 4, y + 4, (w - 8) as u32, (drawn_height - 8) as u32),
                    text: label.clone(),
                    color: COLOR_TEXT,
                    scroll_offset: 0,
                });
            }
        } else if widget.role == "listbox_default" || widget.role == "primary_list" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(100);
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y, w as u32, drawn_height as u32),
                color: Rgba::new(255, 255, 255, 255),
            });
            self.draw_rect_outline(scene, x, y, w, drawn_height, BTN_BORDER);

            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y + 2;
            let mut remaining_h = drawn_height - 4;
            let child_w = w - 4;
            let child_x = x + 2;

            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let child_h = self.draw_widget_recursive(
                        scene,
                        window_id,
                        child,
                        child_x,
                        child_y,
                        child_w,
                        remaining_h,
                    );
                    child_y += child_h;
                    remaining_h -= child_h;
                }
            }
        } else if widget.role == "scrollbar_thumb" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(30);
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y, w as u32, drawn_height as u32),
                color: BTN_FACE,
            });
            self.draw_rect_outline(scene, x, y, w, drawn_height, BTN_BORDER);
        } else if widget.role == "thing_tile" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(100);
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y, w as u32, drawn_height as u32),
                color: BTN_FACE,
            });
            if let Some(label) = &widget.label {
                scene.push(SceneItem::DrawTextBlock {
                    rect: Rect::new(x + 4, y + 4, (w - 8) as u32, (drawn_height - 8) as u32),
                    text: label.clone(),
                    color: COLOR_TEXT,
                    scroll_offset: 0,
                });
            }
        } else if widget.role == "list_item" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(20);
            if let Some(label) = &widget.label {
                scene.push(SceneItem::DrawTextBlock {
                    rect: Rect::new(x + 4, y + 2, (w - 8) as u32, (drawn_height - 4) as u32),
                    text: label.clone(),
                    color: COLOR_TEXT,
                    scroll_offset: 0,
                });
            }
        }

        drawn_height
    }

    fn draw_toolbar_button(
        &self,
        scene: &mut Scene,
        widget: &userland::ui_graph::Widget,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, w.max(0) as u32, h.max(0) as u32),
            color: BTN_FACE,
        });
        self.draw_rect_outline(scene, x, y, w, h, BTN_BORDER);

        if w > 2 && h > 2 {
            let highlight = self.theme.frame_hilight;
            let shadow = self.theme.frame_shadow;
            let inner_width = (w - 2).max(0) as u32;
            let inner_height = (h - 2).max(0) as u32;

            scene.push(SceneItem::FillRect {
                rect: Rect::new(x + 1, y + 1, inner_width, 1),
                color: highlight,
            });
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x + 1, y + 1, 1, inner_height),
                color: highlight,
            });

            scene.push(SceneItem::FillRect {
                rect: Rect::new(x + 1, y + h - 2, inner_width, 1),
                color: shadow,
            });
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x + w - 2, y + 1, 1, inner_height),
                color: shadow,
            });
        }

        if let Some(icon_name) = &widget.icon {
            self.draw_toolbar_icon(scene, icon_name, x, y, w, h);
        }
    }

    fn draw_toolbar_icon(
        &self,
        scene: &mut Scene,
        icon_name: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        let color = BTN_GLYPH;
        let center_offset = |container: i32, item: i32| -> i32 { ((container - item).max(0)) / 2 };

        match icon_name {
            "save" => {
                let icon_size = 12;
                let icon_left = x + center_offset(w, icon_size);
                let icon_top = y + center_offset(h, icon_size);

                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left, icon_top, icon_size as u32, icon_size as u32),
                    color,
                });
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left + 2, icon_top, (icon_size - 4).max(0) as u32, 4),
                    color: BTN_FACE,
                });
            }
            "undo" => {
                let icon_width = 12;
                let icon_height = 6;
                let icon_left = x + center_offset(w, icon_width);
                let icon_top = y + center_offset(h, icon_height);

                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left, icon_top + 2, icon_width as u32, 2),
                    color,
                });
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left, icon_top, 2, icon_height as u32),
                    color,
                });
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left, icon_top, 6, 2),
                    color,
                });
            }
            _ => {
                let icon_width = 8;
                let icon_height = FONT_HEIGHT as i32;
                let icon_left = x + center_offset(w, icon_width);
                let icon_top = y + center_offset(h, icon_height);
                let fallback_char = icon_name.chars().next().unwrap_or('?');

                scene.push(SceneItem::DrawText {
                    origin: (icon_left, icon_top),
                    text: fallback_char.to_string(),
                    color,
                    max_width: Some(w.max(0) as u32),
                });
            }
        }
    }

    fn draw_rect_outline(&self, scene: &mut Scene, x: i32, y: i32, w: i32, h: i32, color: Rgba) {
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, w as u32, 1),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y + h - 1, w as u32, 1),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, 1, h as u32),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x + w - 1, y, 1, h as u32),
            color,
        });
    }

    fn draw_close_button(&self, scene: &mut Scene, layout: &WindowLayout, surface: &WindowSurface) {
        let (btn_x, btn_y, btn_w, btn_h) = close_button_rect(layout);

        // Create temporary buffer
        let mut buffer = vec![0u8; (btn_w * btn_h * 4) as usize];
        let rect = userland::widget_abi::Rect {
            x: 0,
            y: 0,
            width: btn_w as u32,
            height: btn_h as u32,
        };

        ButtonWidget::draw(&surface.close_button_state, &mut buffer, rect);

        // Convert to u32 pixels for Bitmap
        let pixels: Vec<u32> = buffer
            .chunks(4)
            .map(|c| {
                let r = c[0] as u32;
                let g = c[1] as u32;
                let b = c[2] as u32;
                let a = c[3] as u32;
                (a << 24) | (r << 16) | (g << 8) | b
            })
            .collect();

        let bitmap = Arc::new(Bitmap::new(btn_w as usize, btn_h as usize, pixels));

        scene.push(SceneItem::BlitImage {
            rect: Rect::new(btn_x, btn_y, btn_w as u32, btn_h as u32),
            image: bitmap,
            repeat: false,
            offset: (0, 0),
        });
    }

    fn draw_surface_content(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        let Some(surface) = self.windows.get(&window_id) else {
            return;
        };

        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, w as u32, h as u32),
            color: THEME.client_bg,
        });

        if let Some(bmp) = &surface.bitmap {
            scene.push(SceneItem::BlitImage {
                rect: Rect::new(x, y, w as u32, h as u32),
                image: bmp.clone(),
                repeat: false,
                offset: (0, 0),
            });
        } else if !surface.text.is_empty() {
            scene.push(SceneItem::DrawTextBlock {
                rect: Rect::new(x, y, w as u32, h as u32),
                text: surface.text.clone(),
                color: COLOR_TEXT,
                scroll_offset: surface.scroll_y,
            });

            let is_active = surface.window.active || self.active_window == Some(window_id);
            if is_active && surface.caret.visible {
                let cx = surface.caret.x;
                let cy = surface.caret.y;
                let ch = surface.caret.height;

                let draw_cx = x + cx;
                let draw_cy = y + cy - surface.scroll_y;

                if draw_cy + ch >= y && draw_cy < y + h {
                    scene.push(SceneItem::FillRect {
                        rect: Rect::new(draw_cx, draw_cy, surface.caret.width as u32, ch as u32),
                        color: COLOR_TEXT,
                    });
                }
            }
        }
    }

    fn ingest_surface(&mut self, thing: &userland::GraphThing) {
        if thing.kind != canon::SURFACE {
            return;
        }
        let Some(surface) = Surface::load(thing) else {
            return;
        };
        let window_id = surface
            .window
            .or_else(|| thing.fields.get(&canon::SRC).and_then(Value::as_uuid));
        let Some(window_id) = window_id else {
            return;
        };

        let window = load_thing::<Window>(window_id).unwrap_or_else(|| default_window(window_id));
        let entry = self.windows.entry(window_id).or_insert_with(|| {
            let (btn_id, btn_state) = create_close_button();
            WindowSurface {
                window: window.clone(),
                surface_id: None,
                text: String::new(),
                bitmap: None,
                scroll_y: 0,
                scrollbar_widget_id: None,
                caret: Caret::default(),
                close_button_id: btn_id,
                close_button_state: btn_state,
                close_button_bitmap: None,
            }
        });
        entry.window = window;
        entry.surface_id = Some(surface.id);
        entry.text = surface.text;

        if let Some(bytes) = surface.bitmap {
            if let Some(bmp) = decode_bmp(&bytes) {
                entry.bitmap = Some(Arc::new(bmp));
            }
        }

        self.bump_window(window_id);
        self.clamp_scroll_for(window_id);
        self.apply_window_rect_hint(window_id);
    }

    fn ordered_window_ids(&self) -> Vec<Uuid> {
        let mode = &self.modes[self.active_mode];
        let mut ids = Vec::new();
        if let Some(root) = mode.root_window {
            if let Some(w) = self.windows.get(&root) {
                if w.window.visible {
                    ids.push(root);
                }
            }
        }
        for &id in &mode.windows {
            if let Some(w) = self.windows.get(&id) {
                if w.window.visible {
                    ids.push(id);
                }
            }
        }
        ids
    }

    fn visible_window_ids(&self) -> Vec<Uuid> {
        self.ordered_window_ids()
            .into_iter()
            .filter(|id| {
                self.windows
                    .get(id)
                    .map(|surface| surface.surface_id.is_some())
                    .unwrap_or(false)
            })
            .collect()
    }

    pub fn on_mouse_event(&mut self, dx: i64, dy: i64, buttons: u64) {
        if dx != 0 || dy != 0 {
            // println!("Compositor move: dx={} dy={}", dx, dy);
        }

        let geo = self.fb_device.geometry();
        let (width, height) = (geo.width as usize, geo.height as usize);
        let prev_buttons = self.cursor.buttons;
        self.cursor.update(dx, dy, buttons as u8, width, height);

        let left_down = (buttons & 1) != 0;
        let left_was_down = (prev_buttons & 1) != 0;
        let left_pressed = left_down && !left_was_down;
        let left_released = !left_down && left_was_down;

        if left_pressed {
            self.on_pointer_down();
        }

        if left_down {
            self.continue_drag();
            self.continue_widget_interaction();
        } else if left_released {
            if let Some(drag) = &self.drag_state {
                if let DragKind::CloseButton = drag.kind {
                    self.on_close_button_up(drag.window_id);
                }
            }
            self.drag_state = None;
            self.end_widget_interaction();
        } else {
            self.drag_state = None;
        }

        self.update_cursor_kind();
    }

    fn ingest_input_event(&mut self, thing: &userland::GraphThing) {
        // println!("Compositor ingest: {:?}", thing);
        if let Some(kind) = thing.fields.get(&canon::KIND).and_then(|v| v.as_symbol()) {
            if kind == canon::MOVE {
                let dx = thing
                    .fields
                    .get(&canon::DX)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let dy = thing
                    .fields
                    .get(&canon::DY)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                let buttons = thing
                    .fields
                    .get(&canon::BUTTON)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                self.on_mouse_event(dx, dy, buttons);
            }
        }
    }

    fn on_close_button_up(&mut self, window_id: Uuid) {
        // Reset pressed state
        if let Some(surface) = self.windows.get_mut(&window_id) {
            surface.close_button_state.pressed = false;
        }

        // Check if still over button
        let (win_x, win_y, win_w, win_h) = if let Some(surface) = self.windows.get(&window_id) {
            (
                surface.window.x as i32,
                surface.window.y as i32,
                surface.window.width as i32,
                surface.window.height as i32,
            )
        } else {
            return;
        };

        if let Some(layout) = compute_window_layout(win_x, win_y, win_w, win_h) {
            let close_rect = close_button_rect(&layout);
            if point_in_rect(self.cursor.x, self.cursor.y, close_rect) {
                println!("Close button clicked for window {}", window_id);
                let mut props = BTreeMap::new();
                props.insert(canon::VISIBLE, Value::Bool(false));
                self.update_window_props(window_id, props);
            }
        }
    }

    pub fn switch_mode(&mut self, new_mode_idx: usize) {
        if self.active_mode == new_mode_idx {
            return;
        }
        self.active_mode = new_mode_idx;
        self.fb_dirty = true;
    }

    fn ingest_key_event(&mut self, thing: &userland::GraphThing) {
        let key = thing.fields.get(&canon::KEY).and_then(|v| v.as_symbol());
        let down = thing
            .fields
            .get(&canon::DOWN)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let scancode = thing.fields.get(&canon::SCANCODE).and_then(|v| v.as_u64());

        if let Some(key) = key {
            if scancode == Some(0x38)
                || (scancode.is_none()
                    && (key == canon::cc('A', 'L') || key == canon::cc('Y', 'L')))
            {
                self.alt_down = down;
            }

            if scancode == Some(0x2A)
                || scancode == Some(0x36)
                || (scancode.is_none() && key == canon::cc('S', 'F'))
            {
                self.shift_down = down;
            }

            if down {
                if self.alt_down && (key == canon::from_char('t') || key == canon::from_char('T')) {
                    self.tile_windows();
                } else if self.alt_down
                    && (key == canon::from_char('i') || key == canon::from_char('I'))
                {
                    self.debug_layout_mode = !self.debug_layout_mode;
                    self.fb_dirty = true;
                } else if self.alt_down
                    && (key == canon::from_char('o') || key == canon::from_char('O'))
                {
                    self.debug_overlay_mode = !self.debug_overlay_mode;
                    self.fb_dirty = true;
                } else if key.0 >= 0xF001 && key.0 <= 0xF00C {
                    let mode_idx = (key.0 - 0xF001) as usize;
                    self.switch_mode(mode_idx);
                } else if key == canon::cc('T', 'B') {
                    self.handle_tab_focus();
                }
            }

            if down {
                self.handle_scroll_key(key);
            }
        }
    }

    fn handle_tab_focus(&mut self) {
        let Some(active_window_id) = self.active_window else {
            return;
        };

        let mut focusable = Vec::new();
        self.collect_focusable_widgets(active_window_id, &mut focusable);

        if focusable.is_empty() {
            return;
        }

        let current_index = self
            .active_widget
            .and_then(|id| focusable.iter().position(|x| *x == id));

        let next_index = if let Some(idx) = current_index {
            if self.shift_down {
                if idx == 0 {
                    focusable.len() - 1
                } else {
                    idx - 1
                }
            } else {
                (idx + 1) % focusable.len()
            }
        } else {
            0
        };

        let old_widget = self.active_widget;
        let new_widget = focusable[next_index];
        self.active_widget = Some(new_widget);
        self.fb_dirty = true;

        let focused_sym = canon::canon(b'F', b'C', b'S');

        if let Some(old_id) = old_widget {
            if old_id != new_widget {
                let mut updates = graph::map();
                updates.insert(focused_sym, Value::Bool(false));
                graph::fiat(Some(old_id), canon::WIDGET, updates);
            }
        }

        let mut updates = graph::map();
        updates.insert(focused_sym, Value::Bool(true));
        graph::fiat(Some(new_widget), canon::WIDGET, updates);
    }

    fn collect_focusable_widgets(&self, parent_id: Uuid, list: &mut Vec<Uuid>) {
        let mut children: Vec<&userland::ui_graph::Widget> = self
            .widgets
            .values()
            .filter(|w| w.parent == Some(parent_id))
            .collect();

        // Sort by ID for stability (creation order)
        children.sort_by_key(|w| w.id);

        for child in children {
            if child.focusable {
                list.push(child.id);
            }
            self.collect_focusable_widgets(child.id, list);
        }
    }

    fn tile_windows(&mut self) {
        let visible_windows = self.visible_window_ids();

        if visible_windows.is_empty() {
            return;
        }

        let count = visible_windows.len() as i32;
        let mut cols = 1;
        while cols * cols < count {
            cols += 1;
        }
        let rows = (count + cols - 1) / cols;

        let geo = self.fb_device.geometry();
        let screen_w = geo.width as i32;
        let screen_h = geo.height as i32;

        let w = screen_w / cols;
        let h = screen_h / rows;

        for (i, win_id) in visible_windows.iter().enumerate() {
            let row = (i as i32) / cols;
            let col = (i as i32) % cols;

            let x = col * w;
            let y = row * h;

            if let Some(entry) = self.windows.get_mut(win_id) {
                entry.window.x = x.max(0) as u64;
                entry.window.y = y.max(0) as u64;
                entry.window.width = w.max(MIN_WINDOW_WIDTH) as u64;
                entry.window.height = h.max(MIN_WINDOW_HEIGHT) as u64;
            }

            let mut props = BTreeMap::new();
            props.insert(canon::X, Value::U64(x.max(0) as u64));
            props.insert(canon::Y, Value::U64(y.max(0) as u64));
            props.insert(canon::WIDTH, Value::U64(w.max(MIN_WINDOW_WIDTH) as u64));
            props.insert(canon::HEIGHT, Value::U64(h.max(MIN_WINDOW_HEIGHT) as u64));

            self.update_window_props(*win_id, props);
        }
    }

    fn maybe_auto_tile_windows(&mut self) {
        if self.auto_layout_done {
            return;
        }

        let visible_windows = self.visible_window_ids();
        if visible_windows.len() < AUTO_TILE_MIN_WINDOWS {
            return;
        }

        let geo = self.fb_device.geometry();
        // Reserve banner space so tiled windows start below the toolbar.
        let available_height = (geo.height as i32).saturating_sub(AUTO_TILE_TOP_OFFSET);
        if available_height <= 0 {
            return;
        }

        let area = Rect::new(0, AUTO_TILE_TOP_OFFSET, geo.width, available_height as u32);
        self.layout_windows_in_area(area, &visible_windows);

        self.auto_layout_done = true;
        self.fb_dirty = true;
    }

    fn layout_windows_in_area(&mut self, area: Rect, window_ids: &[Uuid]) {
        if window_ids.is_empty() || area.width == 0 || area.height == 0 {
            return;
        }

        let count = window_ids.len();
        let mut cols = 1;
        while cols * cols < count {
            cols += 1;
        }
        let rows = (count + cols - 1) / cols;

        let items: Vec<LayoutItem> = window_ids
            .iter()
            .map(|id| LayoutItem {
                id: *id,
                min_width: MIN_WINDOW_WIDTH as u32,
                min_height: MIN_WINDOW_HEIGHT as u32,
                ..Default::default()
            })
            .collect();

        let spec = LayoutSpec::Grid { rows, cols };
        let rects = layout::layout(area, spec, &items, AUTO_TILE_MARGIN);

        for (win_id, rect) in rects {
            if let Some(entry) = self.windows.get_mut(&win_id) {
                entry.window.x = rect.x.max(0) as u64;
                entry.window.y = rect.y.max(0) as u64;
                entry.window.width = rect.width as u64;
                entry.window.height = rect.height as u64;
            }

            let mut props = BTreeMap::new();
            props.insert(canon::X, Value::U64(rect.x.max(0) as u64));
            props.insert(canon::Y, Value::U64(rect.y.max(0) as u64));
            props.insert(canon::WIDTH, Value::U64(rect.width as u64));
            props.insert(canon::HEIGHT, Value::U64(rect.height as u64));
            self.update_window_props(win_id, props);
        }
    }

    fn continue_widget_interaction(&mut self) {
        if let Some(widget_id) = self.active_widget {
            if let Some(widget) = self.widgets.get(&widget_id) {
                let wx = widget.x.unwrap_or(0) as i32;
                let wy = widget.y.unwrap_or(0) as i32;
                let local_x = self.cursor.x - wx;
                let local_y = self.cursor.y - wy;

                let mut updates = graph::map();
                updates.insert(canon::MOUSE_X, Value::I64(local_x as i64));
                updates.insert(canon::MOUSE_Y, Value::I64(local_y as i64));
                graph::fiat(Some(widget_id), canon::WIDGET, updates);
            }
        }
    }

    fn end_widget_interaction(&mut self) {
        if let Some(widget_id) = self.active_widget {
            let mut updates = graph::map();
            updates.insert(canon::MOUSE_DOWN, Value::Bool(false));
            graph::fiat(Some(widget_id), canon::WIDGET, updates);
            self.active_widget = None;
        }
    }

    fn on_pointer_down(&mut self) {
        if let Some((win_id, win_x, win_y)) = self.find_window_at(self.cursor.x, self.cursor.y) {
            self.set_active_window(Some(win_id));

            let (win_width, win_height) = {
                let Some(surface) = self.windows.get(&win_id) else {
                    return;
                };
                (surface.window.width as i32, surface.window.height as i32)
            };
            let Some(layout) = compute_window_layout(win_x, win_y, win_width, win_height) else {
                return;
            };

            let close_rect = close_button_rect(&layout);
            if point_in_rect(self.cursor.x, self.cursor.y, close_rect) {
                if let Some(surface) = self.windows.get_mut(&win_id) {
                    surface.close_button_state.pressed = true;
                }
                self.drag_state = Some(DragState {
                    window_id: win_id,
                    kind: DragKind::CloseButton,
                });
                return;
            }

            let metrics = {
                let Some(surface) = self.windows.get(&win_id) else {
                    return;
                };
                ContentMetrics::new(surface, &layout)
            };

            if let Some(widget_id) =
                self.hit_test_widgets(win_id, &layout, &metrics, self.cursor.x, self.cursor.y)
            {
                self.active_widget = Some(widget_id);
                if let Some(widget) = self.widgets.get(&widget_id) {
                    let wx = widget.x.unwrap_or(0) as i32;
                    let wy = widget.y.unwrap_or(0) as i32;
                    let local_x = self.cursor.x - wx;
                    let local_y = self.cursor.y - wy;

                    let mut updates = graph::map();
                    updates.insert(canon::MOUSE_X, Value::I64(local_x as i64));
                    updates.insert(canon::MOUSE_Y, Value::I64(local_y as i64));
                    updates.insert(canon::MOUSE_DOWN, Value::Bool(true));
                    graph::fiat(Some(widget_id), canon::WIDGET, updates);

                    if widget.role == ROLE_TOOLBAR_BUTTON {
                        println!("Toolbar button clicked: {}", widget_id);
                        if let Some(action) = &widget.action {
                            println!("Action: {}", action);
                        }
                        return;
                    }
                }
                return;
            }

            if metrics.max_scroll > 0 {
                if let (Some(track), Some(thumb), Some(offset)) = (
                    metrics.scrollbar_track_rect,
                    metrics.scrollbar_thumb_rect,
                    metrics.scrollbar_thumb_offset,
                ) {
                    if rect_contains(&track, self.cursor.x, self.cursor.y) {
                        if rect_contains(&thumb, self.cursor.x, self.cursor.y) {
                            self.drag_state = Some(DragState {
                                window_id: win_id,
                                kind: DragKind::ScrollThumb {
                                    track_height: track.height as i32,
                                    thumb_height: thumb.height as i32,
                                    thumb_offset: offset,
                                    max_scroll: metrics.max_scroll,
                                    start_cursor_y: self.cursor.y,
                                },
                            });
                        } else {
                            let page = metrics.viewport_height.max(SCROLL_STEP_LINE);
                            if self.cursor.y < thumb.y {
                                self.scroll_window_by(win_id, -page);
                            } else {
                                self.scroll_window_by(win_id, page);
                            }
                        }
                        return;
                    }
                }
            }

            if let Some(edges) = hit_test_resize(
                win_x,
                win_y,
                win_width,
                win_height,
                self.cursor.x,
                self.cursor.y,
            ) {
                self.drag_state = Some(DragState {
                    window_id: win_id,
                    kind: DragKind::Resize {
                        edges,
                        start_cursor_x: self.cursor.x,
                        start_cursor_y: self.cursor.y,
                        start_x: win_x,
                        start_y: win_y,
                        start_w: win_width,
                        start_h: win_height,
                    },
                });
                return;
            }

            if self.cursor.y >= layout.title_y && self.cursor.y < layout.title_y + layout.title_h {
                self.drag_state = Some(DragState {
                    window_id: win_id,
                    kind: DragKind::Move {
                        offset_x: self.cursor.x - win_x,
                        offset_y: self.cursor.y - win_y,
                    },
                });
            }
        } else {
            self.set_active_window(None);
        }
    }

    fn continue_drag(&mut self) {
        let Some(drag) = &self.drag_state else {
            return;
        };

        match &drag.kind {
            DragKind::Move { offset_x, offset_y } => {
                let new_x = max(0, self.cursor.x - offset_x);
                let new_y = max(0, self.cursor.y - offset_y);

                if let Some(entry) = self.windows.get_mut(&drag.window_id) {
                    entry.window.x = new_x as u64;
                    entry.window.y = new_y as u64;
                }

                let mut props = BTreeMap::new();
                props.insert(canon::X, Value::U64(new_x as u64));
                props.insert(canon::Y, Value::U64(new_y as u64));

                self.update_window_props(drag.window_id, props);
            }
            DragKind::Resize {
                edges,
                start_cursor_x,
                start_cursor_y,
                start_x,
                start_y,
                start_w,
                start_h,
            } => {
                let dx = self.cursor.x - start_cursor_x;
                let dy = self.cursor.y - start_cursor_y;

                let mut new_x = *start_x;
                let mut new_y = *start_y;
                let mut new_w = *start_w;
                let mut new_h = *start_h;

                if edges.left {
                    let proposed_w = start_w - dx;
                    let clamped_w = max(MIN_WINDOW_WIDTH, proposed_w);
                    let delta = start_w - clamped_w;
                    new_x = start_x + delta;
                    new_w = clamped_w;
                } else if edges.right {
                    new_w = max(MIN_WINDOW_WIDTH, start_w + dx);
                }

                if edges.top {
                    let proposed_h = start_h - dy;
                    let clamped_h = max(MIN_WINDOW_HEIGHT, proposed_h);
                    let delta = start_h - clamped_h;
                    new_y = start_y + delta;
                    new_h = clamped_h;
                } else if edges.bottom {
                    new_h = max(MIN_WINDOW_HEIGHT, start_h + dy);
                }

                if new_x < 0 {
                    let overshoot = -new_x;
                    new_x = 0;
                    new_w = max(new_w + overshoot, MIN_WINDOW_WIDTH);
                }
                if new_y < 0 {
                    let overshoot = -new_y;
                    new_y = 0;
                    new_h = max(new_h + overshoot, MIN_WINDOW_HEIGHT);
                }

                if let Some(entry) = self.windows.get_mut(&drag.window_id) {
                    entry.window.x = new_x as u64;
                    entry.window.y = new_y as u64;
                    entry.window.width = new_w as u64;
                    entry.window.height = new_h as u64;
                }

                let mut props = BTreeMap::new();
                props.insert(canon::X, Value::U64(new_x as u64));
                props.insert(canon::Y, Value::U64(new_y as u64));
                props.insert(canon::WIDTH, Value::U64(new_w as u64));
                props.insert(canon::HEIGHT, Value::U64(new_h as u64));

                self.update_window_props(drag.window_id, props);
                self.clamp_scroll_for(drag.window_id);
            }
            DragKind::ScrollThumb {
                track_height,
                thumb_height,
                thumb_offset,
                max_scroll,
                start_cursor_y,
            } => {
                if *max_scroll <= 0 {
                    return;
                }
                let travel = (*track_height - *thumb_height).max(1);
                if travel <= 0 {
                    return;
                }
                let delta_pixels = self.cursor.y - start_cursor_y;
                let new_thumb_offset = clamp_i32(thumb_offset + delta_pixels, 0, travel);
                let ratio = new_thumb_offset as f32 / travel as f32;
                let new_scroll = ((ratio * *max_scroll as f32) + 0.5) as i32;
                self.set_scroll_offset(drag.window_id, new_scroll);
            }
            DragKind::CloseButton => {}
        }
    }

    fn max_window_z(&self) -> i64 {
        self.windows.values().map(|w| w.window.z).max().unwrap_or(0)
    }

    fn ensure_window_has_unique_z(&mut self, window_id: Uuid, prev_max_z: i64) {
        let Some(entry) = self.windows.get_mut(&window_id) else {
            return;
        };
        if entry.window.z > prev_max_z {
            return;
        }

        let new_z = prev_max_z.saturating_add(1);
        if entry.window.z == new_z {
            return;
        }

        entry.window.z = new_z;
        let mut props = BTreeMap::new();
        props.insert(canon::Z, Value::I64(new_z));
        self.update_window_props(window_id, props);
    }

    fn update_window_props(&self, window_id: Uuid, props: BTreeMap<canon::Symbol, Value>) {
        if props.is_empty() {
            return;
        }
        let req = AbiRequest::PropsSet {
            request: GraphPropsRequest {
                node: window_id,
                props,
            },
        };
        let _ = userland::runtime().call(req);
    }

    fn set_active_window(&mut self, window_id: Option<Uuid>) {
        if self.active_window == window_id {
            return;
        }

        let prev_window = self.active_window;

        if let Some(prev) = self.active_window.take() {
            if let Some(entry) = self.windows.get_mut(&prev) {
                entry.window.active = false;
            }
            let mut props = BTreeMap::new();
            props.insert(canon::ACTIVE, Value::Bool(false));
            self.update_window_props(prev, props);
        }

        if let Some(id) = window_id {
            let new_z = self.max_window_z().saturating_add(1);
            if let Some(entry) = self.windows.get_mut(&id) {
                entry.window.active = true;
                entry.window.z = new_z;
            }

            let mut props = BTreeMap::new();
            props.insert(canon::ACTIVE, Value::Bool(true));
            props.insert(canon::Z, Value::I64(new_z));
            self.update_window_props(id, props);
            self.active_window = Some(id);
            self.bump_window(id);
        } else {
            self.active_window = None;
            self.update_graph_state();
        }

        self.content_dirty = true;
    }

    fn ensure_active_window(&mut self) {
        if let Some(active) = self.active_window {
            if let Some(surface) = self.windows.get(&active) {
                if surface.window.visible {
                    return;
                }
            }
        }

        if let Some((id, _)) = self
            .windows
            .iter()
            .filter(|(_, surface)| surface.window.active && surface.window.visible)
            .max_by_key(|(_, surface)| surface.window.z)
        {
            if self.active_window != Some(*id) {
                self.active_window = Some(*id);
                self.content_dirty = true;
            }
            return;
        }

        if let Some(id) = self.ordered_window_ids().into_iter().last() {
            self.set_active_window(Some(id));
        } else {
            self.set_active_window(None);
        }
    }

    fn cursor_kind_for_edges(edges: &ResizeEdges) -> CursorKind {
        match (edges.left, edges.right, edges.top, edges.bottom) {
            (true, false, true, false) => CursorKind::ResizeNW,
            (false, true, true, false) => CursorKind::ResizeNE,
            (true, false, false, true) => CursorKind::ResizeSW,
            (false, true, false, true) => CursorKind::ResizeSE,
            (true, false, false, false) => CursorKind::ResizeW,
            (false, true, false, false) => CursorKind::ResizeE,
            (false, false, true, false) => CursorKind::ResizeN,
            (false, false, false, true) => CursorKind::ResizeS,
            _ => CursorKind::Arrow,
        }
    }

    fn compute_cursor_kind(&self) -> CursorKind {
        if let Some(drag) = &self.drag_state {
            return match &drag.kind {
                DragKind::Move { .. } => CursorKind::Move,
                DragKind::Resize { edges, .. } => Self::cursor_kind_for_edges(edges),
                DragKind::ScrollThumb { .. } => CursorKind::Move,
                DragKind::CloseButton => CursorKind::Arrow,
            };
        }

        if let Some((win_id, win_x, win_y)) = self.find_window_at(self.cursor.x, self.cursor.y) {
            let Some(surface) = self.windows.get(&win_id) else {
                return CursorKind::Arrow;
            };

            let win_width = surface.window.width as i32;
            let win_height = surface.window.height as i32;

            if let Some(edges) = hit_test_resize(
                win_x,
                win_y,
                win_width,
                win_height,
                self.cursor.x,
                self.cursor.y,
            ) {
                return Self::cursor_kind_for_edges(&edges);
            }

            if let Some(layout) = compute_window_layout(win_x, win_y, win_width, win_height) {
                if self.cursor.y >= layout.title_y
                    && self.cursor.y < layout.title_y + layout.title_h
                {
                    return CursorKind::Move;
                }
            }
        }

        CursorKind::Arrow
    }

    fn update_cursor_kind(&mut self) {
        let kind = self.compute_cursor_kind();
        self.cursor.set_kind(kind);
    }

    fn ingest_cursor(&mut self, thing: &userland::GraphThing) {
        if thing.kind != canon::CURSOR {
            return;
        }
        let x = thing.fields.get(&canon::X).and_then(|v| v.as_i64());
        let y = thing.fields.get(&canon::Y).and_then(|v| v.as_i64());
        let visible = thing.fields.get(&canon::VISIBLE).and_then(|v| v.as_bool());
        let geo = self.fb_device.geometry();
        let (width, height) = (geo.width as usize, geo.height as usize);
        self.cursor.set_from_graph(x, y, visible, width, height);
    }

    fn ingest_widget(&mut self, thing: &userland::GraphThing) {
        if let Some(existing) = self.widgets.get_mut(&thing.id) {
            existing.update(thing);

            if let Some(parent_id) = existing.parent {
                if let Some(scroll_y) = thing.fields.get(&canon::SCROLL_Y).and_then(|v| v.as_i64())
                {
                    if let Some(window) = self.windows.get_mut(&parent_id) {
                        if window.scrollbar_widget_id == Some(existing.id) {
                            window.scroll_y = scroll_y as i32;
                        }
                    }
                }
            }
        } else {
            if let Some(widget) = userland::ui_graph::Widget::load(thing) {
                if let Some(parent_id) = widget.parent {
                    if let Some(scroll_y) =
                        thing.fields.get(&canon::SCROLL_Y).and_then(|v| v.as_i64())
                    {
                        if let Some(window) = self.windows.get_mut(&parent_id) {
                            if window.scrollbar_widget_id == Some(widget.id) {
                                window.scroll_y = scroll_y as i32;
                            }
                        }
                    }
                }
                self.widgets.insert(widget.id, widget);
            }
        }
    }

    fn ingest_window(&mut self, window: Window) {
        let window_id = window.id;
        let is_new = !self.windows.contains_key(&window_id);
        let prev_max_z = if is_new {
            Some(self.max_window_z())
        } else {
            None
        };

        if let Some(entry) = self.windows.get_mut(&window_id) {
            entry.window = window;
        } else {
            let (btn_id, btn_state) = create_close_button();
            self.windows.insert(
                window_id,
                WindowSurface {
                    window,
                    surface_id: None,
                    text: String::new(),
                    bitmap: None,
                    scroll_y: 0,
                    scrollbar_widget_id: None,
                    caret: Caret::default(),
                    close_button_id: btn_id,
                    close_button_state: btn_state,
                    close_button_bitmap: None,
                },
            );
        }

        if is_new {
            let window = &self.windows[&window_id].window;
            let target_mode = window
                .mode_index
                .map(|i| i as usize)
                .unwrap_or(self.active_mode);

            if target_mode < self.modes.len() {
                if window.is_root {
                    self.modes[target_mode].root_window = Some(window_id);
                } else {
                    self.modes[target_mode].windows.push(window_id);
                }
            }
        }

        if let Some(prev_max_z) = prev_max_z {
            self.ensure_window_has_unique_z(window_id, prev_max_z);
        }
        if let Some(target) = self.windows.get(&window_id).and_then(|w| w.window.target) {
            if let Some(entry) = self.windows.get_mut(&window_id) {
                entry.surface_id = Some(target);
            }
        }
        let is_active = self
            .windows
            .get(&window_id)
            .map(|w| w.window.active)
            .unwrap_or(false);
        if is_active {
            self.set_active_window(Some(window_id));
        } else if self.active_window == Some(window_id) {
            self.set_active_window(None);
        } else if is_new {
            self.bump_window(window_id);
        }

        self.clamp_scroll_for(window_id);
        self.maybe_auto_tile_windows();
    }

    fn bump_window(&mut self, window_id: Uuid) {
        // Find which mode contains this window and bump it
        for mode in &mut self.modes {
            if let Some(pos) = mode.windows.iter().position(|w| *w == window_id) {
                mode.windows.remove(pos);
                mode.windows.push(window_id);
                break;
            }
        }
        self.update_graph_state();
    }

    fn draw_background(&self, scene: &mut Scene, width: usize, height: usize) {
        scene.push(SceneItem::BlitImage {
            rect: Rect::new(0, 0, width as u32, height as u32),
            image: self.background.clone(),
            repeat: true,
            offset: (0, 0),
        });
    }

    fn draw_windows(&self, scene: &mut Scene, fb_width: usize, fb_height: usize) {
        for id in self.ordered_window_ids() {
            if let Some(surface) = self.windows.get(&id).cloned() {
                if surface.window.is_root {
                    self.draw_window_frameless(scene, &surface, fb_width, fb_height);
                } else if self.debug_layout_mode {
                    self.draw_debug_window(scene, &surface, fb_width, fb_height);
                } else {
                    self.draw_window(scene, &surface, fb_width, fb_height);
                    if self.debug_overlay_mode {
                        self.draw_debug_window(scene, &surface, fb_width, fb_height);
                    }
                }
            }
        }
    }

    fn draw_debug_window(
        &self,
        scene: &mut Scene,
        surface: &WindowSurface,
        fb_width: usize,
        fb_height: usize,
    ) {
        let w = surface.window.width as usize;
        let h = surface.window.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        let x = min(surface.window.x as usize, fb_width);
        let y = min(surface.window.y as usize, fb_height);
        let is_active = surface.window.active || self.active_window == Some(surface.window.id);

        let border_color = if is_active {
            Rgba::new(0xFF, 0, 0, 0xFF) // Red for active
        } else {
            Rgba::new(0x00, 0, 0xFF, 0xFF) // Blue for inactive
        };

        // Draw bounding box (outline)
        // Top
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, y as i32, w as u32, 2),
            color: border_color,
        });
        // Bottom
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, (y + h - 2) as i32, w as u32, 2),
            color: border_color,
        });
        // Left
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, y as i32, 2, h as u32),
            color: border_color,
        });
        // Right
        scene.push(SceneItem::FillRect {
            rect: Rect::new((x + w - 2) as i32, y as i32, 2, h as u32),
            color: border_color,
        });

        // Draw ID or Title in the center
        let label = alloc::format!("ID: {:?}\nActive: {}", surface.window.id, is_active);
        scene.push(SceneItem::DrawTextBlock {
            rect: Rect::new(x as i32 + 4, y as i32 + 4, (w - 8) as u32, (h - 8) as u32),
            text: label,
            color: border_color,
            scroll_offset: 0,
        });

        // Draw Widgets
        let root_widgets: Vec<Uuid> = self
            .widgets
            .values()
            .filter(|w| w.parent == Some(surface.window.id))
            .map(|w| w.id)
            .collect();

        if root_widgets.is_empty() {
            return;
        }

        // Compute layout area (assume full window for debug)
        let client_x = x as i32;
        let client_y = y as i32;
        let client_w = w as i32;
        let client_h = h as i32;

        let mut y_offset = client_y + TITLE_BAR_HEIGHT as i32;
        let x_offset = client_x + BORDER_THICKNESS;
        let width = client_w - BORDER_THICKNESS * 2;
        let mut remaining_h = client_h - TITLE_BAR_HEIGHT as i32 - BORDER_THICKNESS;

        let mut relative_widgets = Vec::new();
        let mut overlay_widgets = Vec::new();

        for widget_id in root_widgets {
            if let Some(widget) = self.widgets.get(&widget_id) {
                if widget.x.is_some() && widget.y.is_some() {
                    overlay_widgets.push(widget_id);
                } else {
                    relative_widgets.push(widget_id);
                }
            }
        }

        for widget_id in relative_widgets {
            let child_h = self.draw_debug_widget_recursive(
                scene,
                surface.window.id,
                widget_id,
                x_offset,
                y_offset,
                width,
                remaining_h,
            );
            y_offset += child_h;
            remaining_h = remaining_h.saturating_sub(child_h);
        }

        for widget_id in overlay_widgets {
            if let Some(widget) = self.widgets.get(&widget_id) {
                if let (Some(wx), Some(wy), Some(ww), Some(wh)) =
                    (widget.x, widget.y, widget.width, widget.height)
                {
                    self.draw_debug_widget_recursive(
                        scene,
                        surface.window.id,
                        widget_id,
                        client_x + wx as i32,
                        client_y + wy as i32,
                        ww as i32,
                        wh as i32,
                    );
                }
            }
        }
    }

    fn draw_debug_widget_recursive(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        widget_id: Uuid,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> i32 {
        let Some(widget) = self.widgets.get(&widget_id) else {
            return 0;
        };
        if !widget.visible {
            return 0;
        }

        let mut drawn_height = 0;

        if widget.role == ROLE_TOOLBAR {
            drawn_height = TOOLBAR_HEIGHT;
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget_id))
                .map(|w| w.id)
                .collect();

            let mut child_x = x;
            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let btn_w = TOOLBAR_BUTTON_SIZE;
                    let btn_h = TOOLBAR_BUTTON_SIZE;
                    let btn_y = y + (drawn_height - btn_h) / 2;

                    self.draw_debug_widget_box(scene, child, child_x, btn_y, btn_w, btn_h);
                    child_x += btn_w + TOOLBAR_BUTTON_SPACING;
                }
            }
        } else if widget.role == ROLE_CONTAINER_VERTICAL || widget.role == "window_root" {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget_id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y;
            let mut remaining_h = h;

            for child_id in children {
                let child_h = self.draw_debug_widget_recursive(
                    scene,
                    window_id,
                    child_id,
                    x,
                    child_y,
                    w,
                    remaining_h,
                );
                child_y += child_h;
                drawn_height += child_h;
                remaining_h = remaining_h.saturating_sub(child_h);
            }
        } else if widget.role == ROLE_EDITOR_ROOT {
            drawn_height = h;

            let scroll_y = if let Some(surface) = self.windows.get(&window_id) {
                surface.scroll_y
            } else {
                0
            };

            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget_id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y - scroll_y;
            // Give children plenty of space to draw themselves
            let child_available_h = 10000;

            for child_id in children {
                let child_h = self.draw_debug_widget_recursive(
                    scene,
                    window_id,
                    child_id,
                    x,
                    child_y,
                    w,
                    child_available_h,
                );
                child_y += child_h;
            }
        } else {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(32);
        }

        self.draw_debug_widget_box(scene, widget, x, y, w, drawn_height);
        drawn_height
    }

    fn draw_debug_widget_box(
        &self,
        scene: &mut Scene,
        widget: &userland::ui_graph::Widget,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        let color = Rgba::new(0xFF, 0x00, 0xFF, 0x00); // Green
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, w as u32, 2),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y + h - 2, w as u32, 2),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, 2, h as u32),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x + w - 2, y, 2, h as u32),
            color,
        });

        if let Some(bitmap) = &widget.bitmap {
            if let Some(bmp) = decode_bmp(bitmap) {
                scene.push(SceneItem::BlitImage {
                    rect: Rect::new(x, y, w as u32, h as u32),
                    image: Arc::new(bmp),
                    repeat: false,
                    offset: (0, 0),
                });
            }
        }

        if Some(widget.id) == self.active_widget {
            scene.push(SceneItem::HatchRect {
                rect: Rect::new(x, y, w as u32, h as u32),
                color: Rgba::new(0xFF, 0xFF, 0xFF, 0x00), // Yellow
                spacing: 4,
            });
        }
    }

    fn draw_max_mode(&self, scene: &mut Scene, fb_width: usize, fb_height: usize) {
        if let Some(active_id) = self.active_window {
            if let Some(surface) = self.windows.get(&active_id).cloned() {
                if surface.window.visible {
                    self.draw_window_frameless(scene, &surface, fb_width, fb_height);
                    return;
                }
            }
        }

        self.draw_background(scene, fb_width, fb_height);
    }

    fn draw_window_frameless(
        &self,
        scene: &mut Scene,
        surface: &WindowSurface,
        fb_width: usize,
        fb_height: usize,
    ) {
        let w = surface.window.width as usize;
        let h = surface.window.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        let x = min(surface.window.x as usize, fb_width);
        let y = min(surface.window.y as usize, fb_height);

        scene.push(SceneItem::ClipPush {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
        });

        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
            color: self.theme.client_bg,
        });

        if let Some(bmp) = &surface.bitmap {
            scene.push(SceneItem::BlitImage {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                image: bmp.clone(),
                repeat: false,
                offset: (0, 0),
            });
        } else if !surface.text.is_empty() {
            scene.push(SceneItem::DrawTextBlock {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                text: surface.text.clone(),
                color: COLOR_TEXT,
                scroll_offset: surface.scroll_y,
            });
        }

        let has_widgets = self
            .widgets
            .values()
            .any(|w| w.parent == Some(surface.window.id));

        if has_widgets {
            let layout = WindowLayout {
                title_x: 0,
                title_y: 0,
                title_w: 0,
                title_h: 0,
                client_x: x as i32,
                client_y: y as i32,
                client_w: w as i32,
                client_h: h as i32,
            };
            self.draw_widgets(scene, surface.window.id, &layout, layout.client_w);
        }

        if !surface.window.active {
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                color: self.theme.inactive_veil,
            });
        }

        scene.push(SceneItem::ClipPop);
    }

    fn draw_window(
        &self,
        scene: &mut Scene,
        surface: &WindowSurface,
        fb_width: usize,
        fb_height: usize,
    ) {
        let w = surface.window.width as usize;
        let h = surface.window.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        let x = min(surface.window.x as usize, fb_width);
        let y = min(surface.window.y as usize, fb_height);
        let is_active = surface.window.active || self.active_window == Some(surface.window.id);

        let title_color = if is_active {
            self.theme.title_active
        } else {
            self.theme.title_inactive
        };
        let title_text = if is_active {
            self.theme.title_text_active
        } else {
            self.theme.title_text_inactive
        };
        let frame_fill = if is_active {
            self.theme.title_active
        } else {
            self.theme.title_inactive
        };

        // --- Shadow ---
        // Feathered drop shadow (expanding layers)
        // Offset (4, 4)
        let sx = x as i32 + 4;
        let sy = y as i32 + 4;
        let sw = w as u32;
        let sh = h as u32;

        // Core
        scene.push(SceneItem::FillRect {
            rect: Rect::new(sx, sy, sw, sh),
            color: Rgba::new(0x40, 0, 0, 0),
        });
        // Feather 1
        scene.push(SceneItem::FillRect {
            rect: Rect::new(sx - 1, sy - 1, sw + 2, sh + 2),
            color: Rgba::new(0x20, 0, 0, 0),
        });
        // Feather 2
        scene.push(SceneItem::FillRect {
            rect: Rect::new(sx - 2, sy - 2, sw + 4, sh + 4),
            color: Rgba::new(0x10, 0, 0, 0),
        });
        // Feather 3
        scene.push(SceneItem::FillRect {
            rect: Rect::new(sx - 3, sy - 3, sw + 6, sh + 6),
            color: Rgba::new(0x08, 0, 0, 0),
        });

        let Some(layout) = compute_window_layout(x as i32, y as i32, w as i32, h as i32) else {
            return;
        };
        let x0 = x as i32;
        let y0 = y as i32;
        let w_i = w as i32;
        let h_i = h as i32;
        let inner_x0 = x0 + BORDER_OUTER_THICKNESS;
        let inner_y0 = y0 + BORDER_OUTER_THICKNESS;
        let inner_w = w_i - BORDER_OUTER_THICKNESS * 2;
        let inner_h = h_i - BORDER_OUTER_THICKNESS * 2;
        let content_x0 = inner_x0 + BORDER_3D_THICKNESS;
        let content_y0 = inner_y0 + BORDER_3D_THICKNESS;
        let content_w = inner_w - BORDER_3D_THICKNESS * 2;
        let content_h = inner_h - BORDER_3D_THICKNESS * 2;

        scene.push(SceneItem::ClipPush {
            rect: Rect::new(x0, y0, w as u32, h as u32),
        });

        // 1. Outer Border
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x0, y0, w as u32, h as u32),
            color: self.theme.frame_outer,
        });

        // 2. Bevel lines to make the frame pop
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                inner_x0,
                inner_y0,
                inner_w as u32,
                BORDER_3D_THICKNESS as u32,
            ),
            color: self.theme.frame_hilight,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                inner_x0,
                inner_y0,
                BORDER_3D_THICKNESS as u32,
                inner_h as u32,
            ),
            color: self.theme.frame_hilight,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                inner_x0 + inner_w - BORDER_3D_THICKNESS,
                inner_y0,
                BORDER_3D_THICKNESS as u32,
                inner_h as u32,
            ),
            color: self.theme.frame_shadow,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                inner_x0,
                inner_y0 + inner_h - BORDER_3D_THICKNESS,
                inner_w as u32,
                BORDER_3D_THICKNESS as u32,
            ),
            color: self.theme.frame_shadow,
        });

        // 3. Inner Frame (Background / focus ring)
        scene.push(SceneItem::FillRect {
            rect: Rect::new(content_x0, content_y0, content_w as u32, content_h as u32),
            color: frame_fill,
        });

        // 4. Titlebar
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                layout.title_x,
                layout.title_y,
                layout.title_w as u32,
                layout.title_h as u32,
            ),
            color: title_color,
        });

        // Bottom line of titlebar
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                layout.title_x,
                layout.title_y + layout.title_h - 1,
                layout.title_w as u32,
                1,
            ),
            color: self.theme.frame_shadow,
        });

        // 6. Control Buttons
        self.draw_close_button(scene, &layout, surface);

        // 5. Title Text
        let title_max_w = (layout.title_w - TITLE_TEXT_LEFT_PAD - 4).max(0) as u32;

        scene.push(SceneItem::DrawText {
            origin: (
                layout.title_x + TITLE_TEXT_LEFT_PAD,
                layout.title_y + TITLE_TEXT_TOP_OFFSET,
            ),
            text: surface.window.title.clone(),
            color: title_text,
            max_width: Some(title_max_w),
        });

        // 7. Client Area
        let client_y = layout.client_y + 1;
        let client_h = (layout.client_h - 1).max(0);
        let client_rect = Rect::new(
            layout.client_x,
            client_y,
            layout.client_w.max(0) as u32,
            client_h as u32,
        );
        scene.push(SceneItem::FillRect {
            rect: client_rect,
            color: self.theme.client_bg,
        });

        let metrics = ContentMetrics::new(surface, &layout);
        let widget_area_width = if metrics.content_rect.width > 0 {
            metrics.content_rect.width as i32
        } else {
            layout.client_w
        };

        // Check for widgets
        let has_widgets = self
            .widgets
            .values()
            .any(|w| w.parent == Some(surface.window.id));
        if has_widgets {
            // Adjust layout for widgets (remove the +1 offset used for legacy border?)
            // The legacy code adds +1 to client_y.
            // My draw_widgets uses layout.client_y directly.
            // I should probably stick to layout.client_y for widgets.
            self.draw_widgets(scene, surface.window.id, &layout, widget_area_width);
        } else {
            let content_rect = metrics.content_rect;

            if content_rect.width > 0 && content_rect.height > 0 {
                if let Some(bmp) = &surface.bitmap {
                    scene.push(SceneItem::BlitImage {
                        rect: content_rect,
                        image: bmp.clone(),
                        repeat: false,
                        offset: (0, 0),
                    });
                } else if !surface.text.is_empty() {
                    scene.push(SceneItem::DrawTextBlock {
                        rect: content_rect,
                        text: surface.text.clone(),
                        color: COLOR_TEXT,
                        scroll_offset: surface.scroll_y,
                    });

                    // Draw caret
                    if is_active && surface.caret.visible {
                        let cx = content_rect.x + surface.caret.x;
                        let cy = content_rect.y + surface.caret.y - surface.scroll_y;
                        if cy + surface.caret.height >= content_rect.y
                            && cy < content_rect.y + content_rect.height as i32
                        {
                            scene.push(SceneItem::FillRect {
                                rect: Rect::new(
                                    cx,
                                    cy,
                                    surface.caret.width as u32,
                                    surface.caret.height as u32,
                                ),
                                color: COLOR_TEXT,
                            });
                        }
                    }
                }
            }
        }

        self.draw_scrollbar_overlay(scene, surface.window.id, &metrics);

        scene.push(SceneItem::ClipPop);
    }

    fn draw_scrollbar_overlay(&self, scene: &mut Scene, window_id: Uuid, metrics: &ContentMetrics) {
        let track_rect = match metrics.scrollbar_track_rect {
            Some(rect) => rect,
            None => return,
        };

        scene.push(SceneItem::FillRect {
            rect: track_rect,
            color: SCROLLBAR_TRACK_COLOR,
        });

        let thumb_rect = match metrics.scrollbar_thumb_rect {
            Some(rect) => rect,
            None => return,
        };

        let dragging_thumb = matches!(
            &self.drag_state,
            Some(DragState {
                kind: DragKind::ScrollThumb { .. },
                window_id: drag_window,
            }) if *drag_window == window_id
        );

        let thumb_color = if dragging_thumb {
            SCROLLBAR_THUMB_HILIGHT
        } else {
            SCROLLBAR_THUMB_COLOR
        };

        scene.push(SceneItem::FillRect {
            rect: thumb_rect,
            color: thumb_color,
        });

        if thumb_rect.height > 1 {
            scene.push(SceneItem::FillRect {
                rect: Rect::new(thumb_rect.x, thumb_rect.y, thumb_rect.width, 1),
                color: SCROLLBAR_THUMB_HILIGHT,
            });
            scene.push(SceneItem::FillRect {
                rect: Rect::new(
                    thumb_rect.x,
                    thumb_rect.y + thumb_rect.height as i32 - 1,
                    thumb_rect.width,
                    1,
                ),
                color: SCROLLBAR_THUMB_SHADOW,
            });
        }
    }

    fn draw_cursor(&self, scene: &mut Scene, fb_width: usize, fb_height: usize) {
        if !self.cursor.visible {
            return;
        }
        let base_x = clamp_i32(self.cursor.x, 0, fb_width.saturating_sub(1) as i32) as i32;
        let base_y = clamp_i32(self.cursor.y, 0, fb_height.saturating_sub(1) as i32) as i32;
        let icon = self.cursor_sprites.for_kind(self.cursor.kind);

        scene.push(SceneItem::DrawCursor {
            origin: (base_x, base_y),
            sprite: icon.bitmap.clone(),
            hotspot: icon.hotspot,
        });
    }

    pub fn set_cursor(&mut self, x: i32, y: i32, buttons: u8) {
        self.cursor.x = x;
        self.cursor.y = y;
        self.cursor.buttons = buttons;
    }
}

fn default_window(id: Uuid) -> Window {
    Window {
        id,
        title: "window".to_string(),
        x: 32,
        y: 32,
        width: 320,
        height: 200,
        z: 0,
        visible: true,
        target: None,
        active: false,
        is_root: false,
        mode_index: None,
        window_rect: None,
        gap: None,
        flex_direction: None,
        justify_content: None,
        align_items: None,
    }
}

fn clamp_i32(v: i32, min_v: i32, max_v: i32) -> i32 {
    max(min_v, min(v, max_v))
}

struct CursorMask {
    width: usize,
    height: usize,
    hotspot: (i32, i32),
    data: Vec<bool>,
}

impl CursorMask {
    fn new(width: usize, height: usize, hotspot: (i32, i32)) -> Self {
        Self {
            width,
            height,
            hotspot,
            data: vec![false; width * height],
        }
    }

    fn set(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let (ux, uy) = (x as usize, y as usize);
        if ux < self.width && uy < self.height {
            self.data[uy * self.width + ux] = true;
        }
    }

    fn filled(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (ux, uy) = (x as usize, y as usize);
        if ux >= self.width || uy >= self.height {
            return false;
        }
        self.data[uy * self.width + ux]
    }
}

fn fill_triangle(mask: &mut CursorMask, p1: (i32, i32), p2: (i32, i32), p3: (i32, i32)) {
    let min_x = core::cmp::min(p1.0, core::cmp::min(p2.0, p3.0));
    let max_x = core::cmp::max(p1.0, core::cmp::max(p2.0, p3.0));
    let min_y = core::cmp::min(p1.1, core::cmp::min(p2.1, p3.1));
    let max_y = core::cmp::max(p1.1, core::cmp::max(p2.1, p3.1));

    let area_sign = |a: (i32, i32), b: (i32, i32), c: (i32, i32)| -> i32 {
        (a.0 - c.0) * (b.1 - c.1) - (b.0 - c.0) * (a.1 - c.1)
    };

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = (x, y);
            let d1 = area_sign(p, p2, p3);
            let d2 = area_sign(p, p3, p1);
            let d3 = area_sign(p, p1, p2);

            let has_neg = (d1 < 0) || (d2 < 0) || (d3 < 0);
            let has_pos = (d1 > 0) || (d2 > 0) || (d3 > 0);

            if !(has_neg && has_pos) {
                mask.set(x, y);
            }
        }
    }
}

fn cursor_icon_from_mask(mask: CursorMask, fill: Rgba, outline: Rgba, _shadow: Rgba) -> CursorIcon {
    let w = mask.width;
    let h = mask.height;
    let mut pixels = vec![0u32; w * h];

    // 1. Generate shadow map (alpha values)
    let mut shadow_alpha = vec![0u8; w * h];
    let offset_x = 4;
    let offset_y = 4;

    for y in 0..h {
        for x in 0..w {
            if mask.data[y * w + x] {
                let sx = x + offset_x;
                let sy = y + offset_y;
                if sx < w && sy < h {
                    shadow_alpha[sy * w + sx] = 0xA0; // Initial intensity
                }
            }
        }
    }

    // 2. Blur the shadow map (simple box blur, 3 passes)
    for _ in 0..3 {
        let src = shadow_alpha.clone();
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let mut sum: u32 = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        sum +=
                            src[(y as isize + dy) as usize * w + (x as isize + dx) as usize] as u32;
                    }
                }
                shadow_alpha[y * w + x] = (sum / 9) as u8;
            }
        }
    }

    // 3. Composite shadow
    for i in 0..pixels.len() {
        let a = shadow_alpha[i];
        if a > 0 {
            // Black shadow
            pixels[i] = (a as u32) << 24;
        }
    }

    // 4. Composite cursor shape
    for y in 0..h {
        for x in 0..w {
            if !mask.data[y * w + x] {
                continue;
            }
            let neighbors = [
                (x as i32 - 1, y as i32),
                (x as i32 + 1, y as i32),
                (x as i32, y as i32 - 1),
                (x as i32, y as i32 + 1),
            ];
            let is_edge = neighbors.iter().any(|(nx, ny)| !mask.filled(*nx, *ny));
            let color = if is_edge { outline } else { fill };

            // Premultiply alpha
            let a = color.a as u32;
            let r = (color.r as u32 * a) / 255;
            let g = (color.g as u32 * a) / 255;
            let b = (color.b as u32 * a) / 255;
            pixels[y * w + x] = (a << 24) | (r << 16) | (g << 8) | b;
        }
    }

    CursorIcon {
        bitmap: Arc::new(Bitmap::new(mask.width, mask.height, pixels)),
        hotspot: mask.hotspot,
    }
}

fn make_arrow_mask() -> CursorMask {
    let _size = CURSOR_SIZE as i32;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (0, 0));

    // Standard arrow shape scaled 3x
    // Main triangle
    fill_triangle(&mut mask, (0, 0), (0, 45), (33, 33));

    // Stem
    // A quad from (12, 36), (21, 54), (27, 51), (18, 33)
    fill_triangle(&mut mask, (12, 36), (21, 54), (27, 51));
    fill_triangle(&mut mask, (12, 36), (27, 51), (18, 33));

    mask
}

fn make_move_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    // Thicker cross
    for y in (c - 14)..=(c + 14) {
        for x in (c - 5)..=(c + 5) {
            mask.set(x, y);
        }
    }
    for x in (c - 14)..=(c + 14) {
        for y in (c - 5)..=(c + 5) {
            mask.set(x, y);
        }
    }
    // Arrow heads
    for i in 0..14 {
        for dx in -i..=i {
            mask.set(c + dx, c - 15 - i);
            mask.set(c + dx, c + 15 + i);
            mask.set(c - 15 - i, c + dx);
            mask.set(c + 15 + i, c + dx);
        }
    }
    mask
}

fn make_resize_ns_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    for y in (c - 14)..=(c + 14) {
        for x in (c - 5)..=(c + 5) {
            mask.set(x, y);
        }
    }
    for i in 0..14 {
        for dx in -i..=i {
            mask.set(c + dx, c - 15 - i);
            mask.set(c + dx, c + 15 + i);
        }
    }
    mask
}

fn make_resize_ew_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    for x in (c - 14)..=(c + 14) {
        for y in (c - 5)..=(c + 5) {
            mask.set(x, y);
        }
    }
    for i in 0..14 {
        for dy in -i..=i {
            mask.set(c - 15 - i, c + dy);
            mask.set(c + 15 + i, c + dy);
        }
    }
    mask
}

fn make_resize_nw_se_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    for offset in -18..=18 {
        let x = c + offset;
        let y = c + offset;
        for t in -4..=4 {
            mask.set(x + t, y);
        }
    }
    for i in 0..14 {
        for dx in 0..=i {
            mask.set(c - 19 - i, c - 4 + dx);
            mask.set(c - 4 + dx, c - 19 - i);
            mask.set(c + 19 + i, c + 4 - dx);
            mask.set(c + 4 - dx, c + 19 + i);
        }
    }
    mask
}

fn make_resize_ne_sw_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    for offset in -18..=18 {
        let x = c + offset;
        let y = c - offset;
        for t in -4..=4 {
            mask.set(x + t, y);
        }
    }
    for i in 0..14 {
        for dx in 0..=i {
            mask.set(c + 19 + i, c - 4 - dx);
            mask.set(c + 4 + dx, c - 19 - i);
            mask.set(c - 19 - i, c + 4 + dx);
            mask.set(c - 4 - dx, c + 19 + i);
        }
    }
    mask
}

fn build_cursor_sprites() -> CursorSprites {
    let outline = THEME.frame_outer;
    let fill = COLOR_CURSOR_PRIMARY;
    let shadow = COLOR_CURSOR_SHADOW;

    let arrow = cursor_icon_from_mask(make_arrow_mask(), fill, outline, shadow);
    let move_icon = cursor_icon_from_mask(make_move_mask(), fill, outline, shadow);
    let resize_ns = cursor_icon_from_mask(make_resize_ns_mask(), fill, outline, shadow);
    let resize_ew = cursor_icon_from_mask(make_resize_ew_mask(), fill, outline, shadow);
    let resize_nw_se = cursor_icon_from_mask(make_resize_nw_se_mask(), fill, outline, shadow);
    let resize_ne_sw = cursor_icon_from_mask(make_resize_ne_sw_mask(), fill, outline, shadow);

    CursorSprites {
        arrow,
        move_icon,
        resize_ns,
        resize_ew,
        resize_ne_sw,
        resize_nw_se,
    }
}

fn sanitize_fb_info(info: FramebufferGeometry) -> FramebufferGeometry {
    const MAX_DIM: u32 = 4096;
    let width = info.width.clamp(1, MAX_DIM);
    let height = info.height.clamp(1, MAX_DIM);
    let mut pitch = if info.pitch >= width * 4 && info.pitch <= width * 8 {
        info.pitch
    } else {
        width * 4
    };
    if pitch < width {
        pitch = width;
    }
    let bpp = if info.bpp == 24 || info.bpp == 32 {
        info.bpp
    } else {
        32
    };
    FramebufferGeometry {
        width,
        height,
        pitch,
        bpp,
    }
}

fn load_background() -> Arc<Bitmap> {
    let data = include_bytes!("../../clouds.bmp");
    Arc::new(decode_bmp(data).unwrap_or_else(|| fallback_background()))
}

fn fallback_background() -> Bitmap {
    let width = 64;
    let height = 64;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let shade = 0x10 + ((x ^ y) as u32 & 0x3F);
            let color = 0x00050505 * shade;
            pixels.push(color);
        }
    }
    Bitmap::new(width, height, pixels)
}

fn decode_bmp(data: &[u8]) -> Option<Bitmap> {
    if data.len() < 54 || &data[0..2] != b"BM" {
        return None;
    }
    let data_offset = u32::from_le_bytes(data[10..14].try_into().ok()?) as usize;
    let width = i32::from_le_bytes(data[18..22].try_into().ok()?);
    let height = i32::from_le_bytes(data[22..26].try_into().ok()?);
    let planes = u16::from_le_bytes(data[26..28].try_into().ok()?);
    let bpp = u16::from_le_bytes(data[28..30].try_into().ok()?);
    let compression = u32::from_le_bytes(data[30..34].try_into().ok()?);

    if planes != 1 || bpp != 24 || compression != 0 {
        return None;
    }

    let width_u = width.unsigned_abs() as usize;
    let height_u = height.unsigned_abs() as usize;
    let stride = ((width_u * 3 + 3) / 4) * 4;

    if data_offset + stride.saturating_mul(height_u) > data.len() {
        return None;
    }

    let mut pixels = vec![0u32; width_u * height_u];
    println!(
        "Allocated pixels at {:p} size {}x{}",
        pixels.as_ptr(),
        width_u,
        height_u
    );
    for row in 0..height_u {
        let src_row = if height > 0 { height_u - 1 - row } else { row };
        let src_start = data_offset + src_row * stride;
        for col in 0..width_u {
            let idx = src_start + col * 3;
            if idx + 3 > data.len() {
                break;
            }
            let b = data[idx] as u32;
            let g = data[idx + 1] as u32;
            let r = data[idx + 2] as u32;
            pixels[row * width_u + col] = (r << 16) | (g << 8) | b;
        }
    }

    Some(Bitmap::new(width_u, height_u, pixels))
}

#[derive(Clone, Copy, Debug)]
pub struct FramebufferTarget {
    pub info: FramebufferGeometry,
    pub addr: *mut u32,
    pub len_bytes: usize,
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_scene_commands() {
        let bitmap = Arc::new(Bitmap::new(1, 1, vec![0xff00ff00]));
        let mut scene = Scene::new(10, 10);
        scene.push(SceneItem::Clear { color: CLEAR_COLOR });
        scene.push(SceneItem::BlitImage {
            rect: Rect::new(0, 0, 10, 10),
            image: bitmap,
            repeat: true,
            offset: (0, 0),
        });
        assert_eq!(scene.items().len(), 2);
    }

    #[test]
    fn bitmap_pixel_accessor_bounds_checks() {
        let bmp = Bitmap::new(2, 2, vec![0x00000001, 0x00000002, 0x00000003, 0x00000004]);

        assert_eq!(bmp.pixel(0, 0), Some(0x00000001));
        assert_eq!(bmp.pixel(1, 0), Some(0x00000002));
        assert_eq!(bmp.pixel(0, 1), Some(0x00000003));
        assert_eq!(bmp.pixel(1, 1), Some(0x00000004));

        assert_eq!(bmp.pixel(2, 0), None);
        assert_eq!(bmp.pixel(0, 2), None);
        assert_eq!(bmp.pixel(2, 2), None);
    }

    #[cfg(feature = "host")]
    #[test]
    fn svg_backend_renders() {
        let mut renderer = SvgRenderer::new();
        let mut scene = Scene::new(10, 10);
        scene.push(SceneItem::Clear {
            color: Rgba::opaque(0, 0, 0),
        });
        let xml = renderer.render(&scene);
        assert!(xml.contains("<svg"));
    }

    #[cfg(feature = "host")]
    #[test]
    fn color_layout_matches_old_values() {
        assert_eq!(THEME.title_active.to_u32(), 0xffc6d8ff);
        assert_eq!(CLEAR_COLOR.to_u32(), 0xff000000);
    }
}

#[cfg(feature = "kernel_standalone")]
pub trait RunnableApp {
    fn tick(&mut self, ctx: &mut userland::app::AppContext<'_>, tick: u64);
    fn on_event(&mut self, ctx: &mut userland::app::AppContext<'_>, ev: userland::AppEvent);
}

#[cfg(feature = "kernel_standalone")]
impl<T: userland::app::App> RunnableApp for T {
    fn tick(&mut self, ctx: &mut userland::app::AppContext<'_>, tick: u64) {
        self.tick(ctx, tick)
    }
    fn on_event(&mut self, ctx: &mut userland::app::AppContext<'_>, ev: userland::AppEvent) {
        self.on_event(ctx, ev)
    }
}

#[cfg(feature = "kernel_standalone")]
struct RunningAppInstance {
    app: alloc::boxed::Box<dyn RunnableApp>,
    state: userland::app::AppState,
    app_id: usize,
}

#[cfg(feature = "kernel_standalone")]
static PENDING_SPAWNS: spin::Mutex<Vec<String>> = spin::Mutex::new(Vec::new());

#[cfg(feature = "kernel_standalone")]
pub fn request_spawn(name: &str) {
    PENDING_SPAWNS.lock().push(name.to_string());
}
