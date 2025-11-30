#![cfg_attr(not(feature = "host"), no_std)]

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
use userland::graph::GraphPropsRequest;
use userland::{
    canon, load_thing, println, AbiRequest, AppEvent, FramebufferGeometry, NodePattern, Surface,
    Thingable, Value, WatchId, WatchManager, Window,
};
use uuid::Uuid;

mod framebuffer_backend;

pub use framebuffer_backend::{BitmapFramebufferDevice, BitmapRenderer};

const FONT_HEIGHT: usize = 16;
const TITLE_BAR_HEIGHT: usize = 32;
const BORDER_OUTER_THICKNESS: i32 = 1;
const BORDER_3D_THICKNESS: i32 = 1;
const BORDER_THICKNESS: i32 = BORDER_OUTER_THICKNESS + BORDER_3D_THICKNESS;
const RESIZE_MARGIN: i32 = 6;
const RESIZE_CORNER_SIZE: i32 = 8;
const CORNER_RADIUS: i32 = 0;
const MIN_WINDOW_WIDTH: i32 = 140;
const MIN_WINDOW_HEIGHT: i32 = 100;
const CLOSE_BUTTON_SIZE: i32 = 12;
const CLOSE_BUTTON_MARGIN_RIGHT: i32 = 6;
const CLOSE_BUTTON_MARGIN_TOP: i32 = 10;
const TITLE_TEXT_LEFT_PAD: i32 = 8;
const TITLE_TEXT_TOP_OFFSET: i32 = 8;
const CURSOR_SIZE: usize = 98;
const SCROLLBAR_WIDTH: i32 = 24; // WCAG 2.2 SC 2.5.8 requires >=24px pointer targets (W3C Oct 2023).
const SCROLLBAR_GAP: i32 = 4;
const SCROLLBAR_MIN_THUMB: i32 = 32; // Keeps the thumb graspable per WCAG 2.5.5 Target Size (Enhanced).
const SCROLL_STEP_LINE: i32 = FONT_HEIGHT as i32;
const SCROLLBAR_TOTAL_RESERVE: i32 = SCROLLBAR_WIDTH + SCROLLBAR_GAP;

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
}

const THEME: Theme = Theme {
    frame_outer: Rgba::new(0xff, 0x5A, 0x6A, 0x8A),
    frame_light: Rgba::new(0xff, 0xE6, 0xED, 0xF7),
    frame_hilight: Rgba::new(0xff, 0xFF, 0xFF, 0xFF),
    frame_shadow: Rgba::new(0xff, 0x9A, 0xA7, 0xC5),
    title_active: Rgba::new(0xff, 0xC6, 0xD8, 0xFF),
    title_inactive: Rgba::new(0xff, 0xE3, 0xEA, 0xF8),
    title_text_active: Rgba::new(0xff, 0x24, 0x33, 0x4F),
    title_text_inactive: Rgba::new(0xff, 0x6A, 0x74, 0x8A),
    client_bg: Rgba::new(0xff, 0xFD, 0xFB, 0xF7),
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
    fn frame_info(&self) -> Option<FrameInfo> {
        None
    }
}

pub trait RendererBackend {
    type Output<'a>
    where
        Self: 'a;
    fn render<'a>(&'a mut self, scene: &Scene) -> Self::Output<'a>;
}

#[derive(Clone, Debug)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pixels: Arc<[u32]>,
}

impl Bitmap {
    pub fn new(width: usize, height: usize, pixels: Vec<u32>) -> Self {
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

#[derive(Clone)]
struct WindowSurface {
    window: Window,
    surface_id: Option<Uuid>,
    text: String,
    bitmap: Option<Arc<Bitmap>>,
    scroll_y: i32,
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

pub struct Compositor<F, R> {
    frame_no: u64,
    fb_device: F,
    renderer: R,
    windows: BTreeMap<Uuid, WindowSurface>,
    window_order: Vec<Uuid>,
    watch_surfaces: Option<WatchId>,
    watch_windows: Option<WatchId>,
    watch_mouse: Option<WatchId>,
    watch_keyboard: Option<WatchId>,
    watch_cursor: Option<WatchId>,
    watch_fb: Option<WatchId>,
    fb_id: Option<Uuid>,
    fb_dirty: bool,
    cursor: CursorState,
    cursor_sprites: CursorSprites,
    active_window: Option<Uuid>,
    theme: Theme,
    background: Arc<Bitmap>,
    drag_state: Option<DragState>,
    alt_down: bool,
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

        Self {
            frame_no: 0,
            fb_device,
            renderer,
            windows: BTreeMap::new(),
            window_order: Vec::new(),
            watch_surfaces: None,
            watch_windows: None,
            watch_mouse: None,
            watch_keyboard: None,
            watch_cursor: None,
            watch_fb: None,
            fb_id: None,
            fb_dirty: false,
            cursor: CursorState::new(width, height),
            cursor_sprites,
            active_window: None,
            theme,
            background,
            drag_state: None,
            alt_down: false,
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

        let surface_watch = watch_manager.register_pattern(app_id, surface_pattern.clone());
        let window_watch = watch_manager.register_pattern(app_id, window_pattern.clone());
        let cursor_watch = watch_manager.register_pattern(app_id, cursor_pattern.clone());
        let fb_watch = watch_manager.register_pattern(app_id, fb_pattern.clone());
        let mouse_watch = watch_manager.register_pattern(app_id, mouse_pattern);
        let keyboard_watch = watch_manager.register_pattern(app_id, keyboard_pattern);

        let mut comp = Self::new(fb_device, renderer);
        comp.watch_surfaces = Some(surface_watch);
        comp.watch_windows = Some(window_watch);
        comp.watch_mouse = Some(mouse_watch);
        comp.watch_keyboard = Some(keyboard_watch);
        comp.watch_cursor = Some(cursor_watch);
        comp.watch_fb = Some(fb_watch);

        for thing in userland::graph::get_nodes(window_pattern) {
            if let Some(window) = Window::load(&thing) {
                comp.ingest_window(window);
            }
        }

        for thing in userland::graph::get_nodes(surface_discovery) {
            comp.ingest_surface(&thing);
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
                    }
                } else if Some(*watch) == self.watch_cursor {
                    self.ingest_cursor(thing);
                } else if Some(*watch) == self.watch_fb {
                    if thing.kind == canon::DISPLAY_FRAMEBUFFER {
                        self.fb_id = Some(thing.id);
                        self.fb_dirty = true;
                    }
                }
            }
            AppEvent::Edge { .. } => {}
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

        let mut scene = Scene::new(width as u32, height as u32);
        scene.push(SceneItem::Clear { color: CLEAR_COLOR });
        self.draw_background(&mut scene, width, height);
        self.draw_windows(&mut scene, width, height);
        self.draw_cursor(&mut scene, width, height);

        let frame = self.renderer.render(&scene);
        self.fb_device.present(frame);
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
        let entry = self.windows.entry(window_id).or_insert(WindowSurface {
            window: window.clone(),
            surface_id: None,
            text: String::new(),
            bitmap: None,
            scroll_y: 0,
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
    }

    fn ordered_window_ids(&self) -> Vec<Uuid> {
        let mut ordered: Vec<Uuid> = self
            .windows
            .iter()
            .filter_map(|(id, surface)| surface.window.visible.then_some(*id))
            .collect();

        ordered.sort_by(|a, b| {
            use core::cmp::Ordering;
            let a_surface = self
                .windows
                .get(a)
                .expect("ordered window missing from compositor state");
            let b_surface = self
                .windows
                .get(b)
                .expect("ordered window missing from compositor state");

            let z_cmp = a_surface.window.z.cmp(&b_surface.window.z);
            if z_cmp != Ordering::Equal {
                return z_cmp;
            }

            let idx = |id: &Uuid| {
                self.window_order
                    .iter()
                    .position(|w| w == id)
                    .unwrap_or(usize::MAX)
            };
            idx(a).cmp(&idx(b))
        });

        ordered
    }

    fn ingest_input_event(&mut self, thing: &userland::GraphThing) {
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
                } else if left_released {
                    self.drag_state = None;
                } else {
                    self.drag_state = None;
                }

                self.update_cursor_kind();
            }
        }
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
            if down
                && self.alt_down
                && (key == canon::from_char('t') || key == canon::from_char('T'))
            {
                self.tile_windows();
            }
            if down {
                self.handle_scroll_key(key);
            }
        }
    }

    fn tile_windows(&mut self) {
        let visible_windows: Vec<Uuid> = self
            .ordered_window_ids()
            .into_iter()
            .filter(|id| {
                self.windows
                    .get(id)
                    .map(|w| w.window.visible)
                    .unwrap_or(false)
            })
            .collect();

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

    fn on_pointer_down(&mut self) {
        if let Some((win_id, win_x, win_y)) = self.find_window_at(self.cursor.x, self.cursor.y) {
            self.set_active_window(Some(win_id));

            let Some(surface) = self.windows.get(&win_id) else {
                return;
            };
            let win_width = surface.window.width as i32;
            let win_height = surface.window.height as i32;
            let Some(layout) = compute_window_layout(win_x, win_y, win_width, win_height) else {
                return;
            };

            let close_rect = close_button_rect(&layout);
            if point_in_rect(self.cursor.x, self.cursor.y, close_rect) {
                println!("Close button clicked for window {}", win_id);
                let mut props = BTreeMap::new();
                props.insert(canon::VISIBLE, Value::Bool(false));
                self.update_window_props(win_id, props);
                return;
            }

            let metrics = ContentMetrics::new(surface, &layout);
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
        }
    }

    fn max_window_z(&self) -> i64 {
        self.windows.values().map(|w| w.window.z).max().unwrap_or(0)
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
            self.bump_window(id);
            self.active_window = Some(id);
        }
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
            self.active_window = Some(*id);
            return;
        }

        if let Some(id) = self.ordered_window_ids().into_iter().last() {
            self.set_active_window(Some(id));
        } else {
            self.active_window = None;
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

    fn ingest_window(&mut self, window: Window) {
        let window_id = window.id;
        let is_new = !self.windows.contains_key(&window_id);

        if let Some(entry) = self.windows.get_mut(&window_id) {
            entry.window = window;
        } else {
            self.windows.insert(
                window_id,
                WindowSurface {
                    window,
                    surface_id: None,
                    text: String::new(),
                    bitmap: None,
                    scroll_y: 0,
                },
            );
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
            self.active_window = Some(window_id);
            self.bump_window(window_id);
        } else if self.active_window == Some(window_id) {
            self.active_window = None;
        } else if is_new {
            self.bump_window(window_id);
        }

        self.clamp_scroll_for(window_id);
    }

    fn bump_window(&mut self, window_id: Uuid) {
        if let Some(pos) = self.window_order.iter().position(|w| *w == window_id) {
            self.window_order.remove(pos);
        }
        self.window_order.push(window_id);
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
                self.draw_window(scene, &surface, fb_width, fb_height);
            }
        }
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
        let (btn_x, btn_y, btn_w, btn_h) = close_button_rect(&layout);

        // Dimpled look (recessed)
        // Top/Left Shadow
        scene.push(SceneItem::FillRect {
            rect: Rect::new(btn_x, btn_y, btn_w as u32, 1),
            color: self.theme.frame_shadow,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(btn_x, btn_y, 1, btn_h as u32),
            color: self.theme.frame_shadow,
        });
        // Bottom/Right Highlight
        scene.push(SceneItem::FillRect {
            rect: Rect::new(btn_x, btn_y + btn_h - 1, btn_w as u32, 1),
            color: self.theme.frame_hilight,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(btn_x + btn_w - 1, btn_y, 1, btn_h as u32),
            color: self.theme.frame_hilight,
        });
        // Face
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                btn_x + 1,
                btn_y + 1,
                btn_w.saturating_sub(2) as u32,
                btn_h.saturating_sub(2) as u32,
            ),
            color: BTN_FACE,
        });

        // Close button dot
        scene.push(SceneItem::FillRect {
            rect: Rect::new(btn_x + btn_w / 2 - 2, btn_y + btn_h / 2 - 2, 4, 4),
            color: BTN_GLYPH,
        });

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
        let content_rect = metrics.content_rect;

        if content_rect.width > 0 && content_rect.height > 0 {
            if let Some(bmp) = &surface.bitmap {
                scene.push(SceneItem::BlitImage {
                    rect: content_rect,
                    image: bmp.clone(),
                    repeat: false,
                    offset: (0, metrics.scroll_offset),
                });
            }

            scene.push(SceneItem::DrawTextBlock {
                rect: content_rect,
                text: surface.text.clone(),
                color: COLOR_TEXT,
                scroll_offset: metrics.scroll_offset,
            });
        } else if let Some(bmp) = &surface.bitmap {
            scene.push(SceneItem::BlitImage {
                rect: client_rect,
                image: bmp.clone(),
                repeat: false,
                offset: (0, metrics.scroll_offset),
            });
        }

        if let Some(track) = metrics.scrollbar_track_rect {
            scene.push(SceneItem::FillRect {
                rect: track,
                color: SCROLLBAR_TRACK_COLOR,
            });
            if let Some(thumb) = metrics.scrollbar_thumb_rect {
                scene.push(SceneItem::FillRect {
                    rect: thumb,
                    color: SCROLLBAR_THUMB_COLOR,
                });
                // Thumb highlight/shadow improve visual affordance (contrast >=3:1 per WCAG 2.2 SC 1.4.3).
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(thumb.x, thumb.y, thumb.width, 1),
                    color: SCROLLBAR_THUMB_HILIGHT,
                });
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(thumb.x, thumb.y + thumb.height as i32 - 1, thumb.width, 1),
                    color: SCROLLBAR_THUMB_SHADOW,
                });
            }
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
            pixels[y * w + x] = color.to_u32();
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
