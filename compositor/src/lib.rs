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

use userland::graph::GraphPropsRequest;
use userland::{
    canon, load_thing, println, AbiRequest, AppEvent, FramebufferGeometry, NodePattern, Surface,
    Thingable, Value, WatchId, WatchManager, Window,
};
use uuid::Uuid;

mod framebuffer_backend;

pub use framebuffer_backend::{BitmapFramebufferDevice, BitmapRenderer};

const FONT_HEIGHT: usize = 16;
const TITLE_BAR_HEIGHT: usize = 24;
const BORDER_OUTER_THICKNESS: i32 = 1;
const BORDER_3D_THICKNESS: i32 = 1;
const BORDER_THICKNESS: i32 = BORDER_OUTER_THICKNESS + BORDER_3D_THICKNESS;
const RESIZE_MARGIN: i32 = 6;
const RESIZE_CORNER_SIZE: i32 = 8;
const CORNER_RADIUS: i32 = 4;
const MIN_WINDOW_WIDTH: i32 = 140;
const MIN_WINDOW_HEIGHT: i32 = 100;
const CLOSE_BUTTON_SIZE: i32 = 12;
const CLOSE_BUTTON_MARGIN_LEFT: i32 = 6;
const CLOSE_BUTTON_MARGIN_TOP: i32 = 5;
const TITLE_TEXT_LEFT_PAD: i32 = CLOSE_BUTTON_MARGIN_LEFT + CLOSE_BUTTON_SIZE + 6;
const TITLE_TEXT_TOP_OFFSET: i32 = 5;

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

fn point_in_rect(x: i32, y: i32, rect: (i32, i32, i32, i32)) -> bool {
    let (rx, ry, rw, rh) = rect;
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

fn close_button_rect(layout: &WindowLayout) -> (i32, i32, i32, i32) {
    let x = layout.title_x + CLOSE_BUTTON_MARGIN_LEFT;
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

        let surface_watch = watch_manager.register_pattern(app_id, surface_pattern.clone());
        let window_watch = watch_manager.register_pattern(app_id, window_pattern.clone());
        let cursor_watch = watch_manager.register_pattern(app_id, cursor_pattern.clone());
        let fb_watch = watch_manager.register_pattern(app_id, fb_pattern.clone());
        let mouse_watch = watch_manager.register_pattern(app_id, mouse_pattern);

        let mut comp = Self::new(fb_device, renderer);
        comp.watch_surfaces = Some(surface_watch);
        comp.watch_windows = Some(window_watch);
        comp.watch_mouse = Some(mouse_watch);
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

    fn on_pointer_down(&mut self) {
        if let Some((win_id, win_x, win_y)) = self.find_window_at(self.cursor.x, self.cursor.y) {
            self.set_active_window(Some(win_id));

            let Some(surface) = self.windows.get(&win_id) else {
                return;
            };
            let win_width = surface.window.width as i32;
            let win_height = surface.window.height as i32;
            let layout = compute_window_layout(win_x, win_y, win_width, win_height);

            if let Some(layout) = &layout {
                let close_rect = close_button_rect(layout);
                if point_in_rect(self.cursor.x, self.cursor.y, close_rect) {
                    println!("Close button clicked for window {}", win_id);
                    let mut props = BTreeMap::new();
                    props.insert(canon::VISIBLE, Value::Bool(false));
                    self.update_window_props(win_id, props);
                    return;
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

            if let Some(layout) = layout {
                if self.cursor.y >= layout.title_y
                    && self.cursor.y < layout.title_y + layout.title_h
                {
                    self.drag_state = Some(DragState {
                        window_id: win_id,
                        kind: DragKind::Move {
                            offset_x: self.cursor.x - win_x,
                            offset_y: self.cursor.y - win_y,
                        },
                    });
                }
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
        } else if self.active_window == Some(window_id) {
            self.active_window = None;
        }
        self.bump_window(window_id);
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
        // Windows 2000-style offset with soft feather
        let shadow_color_1 = Rgba::new(0x10, 0, 0, 0);
        let shadow_color_2 = Rgba::new(0x10, 0, 0, 0);
        let shadow_color_3 = Rgba::new(0x20, 0, 0, 0);

        // Layer 1 (Outermost)
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32 + 4, y as i32 + 4, w as u32, h as u32),
            color: shadow_color_1,
        });
        // Layer 2
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                x as i32 + 5,
                y as i32 + 5,
                (w as u32).saturating_sub(2),
                (h as u32).saturating_sub(2),
            ),
            color: shadow_color_2,
        });
        // Layer 3 (Core)
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                x as i32 + 6,
                y as i32 + 6,
                (w as u32).saturating_sub(4),
                (h as u32).saturating_sub(4),
            ),
            color: shadow_color_3,
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

        scene.push(SceneItem::FillRect {
            rect: Rect::new(btn_x, btn_y, btn_w as u32, btn_h as u32),
            color: BTN_BORDER,
        });
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
            rect: Rect::new(btn_x + btn_w / 2 - 3, btn_y + btn_h / 2 - 3, 6, 6),
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
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                layout.client_x,
                client_y,
                layout.client_w as u32,
                client_h as u32,
            ),
            color: self.theme.client_bg,
        });

        if let Some(bmp) = &surface.bitmap {
            scene.push(SceneItem::BlitImage {
                rect: Rect::new(
                    layout.client_x,
                    client_y,
                    layout.client_w as u32,
                    client_h as u32,
                ),
                image: bmp.clone(),
                repeat: true,
                offset: (0, 0),
            });
        }

        scene.push(SceneItem::DrawTextBlock {
            rect: Rect::new(
                layout.client_x,
                client_y,
                layout.client_w as u32,
                client_h as u32,
            ),
            text: surface.text.clone(),
            color: COLOR_TEXT,
        });

        self.mask_rounded_corners(scene, x as i32, y as i32, w as u32, h as u32);
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

    fn mask_rounded_corners(&self, scene: &mut Scene, x: i32, y: i32, w: u32, h: u32) {
        let r = CORNER_RADIUS as u32;
        if r == 0 || w < r || h < r {
            return;
        }
        let positions = [
            (x, y),
            (x + w as i32 - r as i32, y),
            (x, y + h as i32 - r as i32),
            (x + w as i32 - r as i32, y + h as i32 - r as i32),
        ];

        for (cx, cy) in positions {
            scene.push(SceneItem::BlitImage {
                rect: Rect::new(cx, cy, r, r),
                image: self.background.clone(),
                repeat: true,
                offset: (cx, cy),
            });
        }
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

fn cursor_icon_from_mask(mask: CursorMask, fill: Rgba, outline: Rgba, shadow: Rgba) -> CursorIcon {
    let mut pixels = vec![0u32; mask.width * mask.height];

    // Shadow pass
    for y in 0..mask.height {
        for x in 0..mask.width {
            if !mask.data[y * mask.width + x] {
                continue;
            }
            let sx = x + 1;
            let sy = y + 1;
            if sx < mask.width && sy < mask.height {
                pixels[sy * mask.width + sx] = shadow.to_u32();
            }
        }
    }

    // Outline + fill
    for y in 0..mask.height {
        for x in 0..mask.width {
            if !mask.data[y * mask.width + x] {
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
            pixels[y * mask.width + x] = color.to_u32();
        }
    }

    CursorIcon {
        bitmap: Arc::new(Bitmap::new(mask.width, mask.height, pixels)),
        hotspot: mask.hotspot,
    }
}

fn make_arrow_mask() -> CursorMask {
    let width = 32;
    let height = 32;
    let mut mask = CursorMask::new(width, height, (0, 0));

    // Broad triangle head
    for y in 0..22 {
        let max_x = core::cmp::min(2 * y + 4, width as i32 - 1);
        for x in 0..=max_x {
            mask.set(x, y);
        }
    }

    // Fatter stem
    for y in 14..height as i32 {
        for x in 12..18 {
            mask.set(x, y);
        }
    }

    mask
}

fn make_move_mask() -> CursorMask {
    let mut mask = CursorMask::new(28, 28, (14, 14));
    let c = 14;
    for y in 6..=22 {
        for x in c - 3..=c + 3 {
            mask.set(x, y);
        }
    }
    for x in 6..=22 {
        for y in c - 3..=c + 3 {
            mask.set(x, y);
        }
    }
    for i in 0..6 {
        for dx in -i..=i {
            mask.set(c + dx, 5 - i);
            mask.set(c + dx, 22 + i);
            mask.set(5 - i, c + dx);
            mask.set(22 + i, c + dx);
        }
    }
    mask
}

fn make_resize_ns_mask() -> CursorMask {
    let mut mask = CursorMask::new(28, 28, (14, 14));
    let c = 14;
    for y in 7..=21 {
        for x in c - 3..=c + 3 {
            mask.set(x, y);
        }
    }
    for i in 0..6 {
        for dx in -i..=i {
            mask.set(c + dx, 6 - i);
            mask.set(c + dx, 21 + i);
        }
    }
    mask
}

fn make_resize_ew_mask() -> CursorMask {
    let mut mask = CursorMask::new(28, 28, (14, 14));
    let c = 14;
    for x in 7..=21 {
        for y in c - 3..=c + 3 {
            mask.set(x, y);
        }
    }
    for i in 0..6 {
        for dy in -i..=i {
            mask.set(6 - i, c + dy);
            mask.set(21 + i, c + dy);
        }
    }
    mask
}

fn make_resize_nw_se_mask() -> CursorMask {
    let mut mask = CursorMask::new(30, 30, (15, 15));
    let c = 15;
    for offset in -8..=8 {
        let x = c + offset;
        let y = c + offset;
        for t in -2..=2 {
            mask.set(x + t, y);
        }
    }
    for i in 0..6 {
        for dx in 0..=i {
            mask.set(c - 9 - i, c - 2 + dx);
            mask.set(c - 2 + dx, c - 9 - i);
            mask.set(c + 9 + i, c + 2 - dx);
            mask.set(c + 2 - dx, c + 9 + i);
        }
    }
    mask
}

fn make_resize_ne_sw_mask() -> CursorMask {
    let mut mask = CursorMask::new(30, 30, (15, 15));
    let c = 15;
    for offset in -8..=8 {
        let x = c + offset;
        let y = c - offset;
        for t in -2..=2 {
            mask.set(x + t, y);
        }
    }
    for i in 0..6 {
        for dx in 0..=i {
            mask.set(c + 9 + i, c - 2 - dx);
            mask.set(c + 2 + dx, c - 9 - i);
            mask.set(c - 9 - i, c + 2 + dx);
            mask.set(c - 2 - dx, c + 9 + i);
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
