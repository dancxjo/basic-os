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

use userland::{
    canon, load_thing, println, AppEvent, NodePattern, Surface, Thingable, Value, WatchId,
    WatchManager, Window,
};
use uuid::Uuid;

mod framebuffer_backend;
#[cfg(feature = "host")]
mod svg_backend;

pub use framebuffer_backend::FramebufferBackend;
#[cfg(feature = "host")]
pub use svg_backend::SvgBackend;

const FONT_HEIGHT: usize = 16;
const TITLE_BAR_HEIGHT: usize = FONT_HEIGHT + 4;
const BORDER_THICKNESS: usize = 2;
const WINDOW_PADDING: usize = 6;
const CURSOR_SIZE: usize = 16;
const CURSOR_MASK: [u16; CURSOR_SIZE] = [
    0b1000000000000000,
    0b1100000000000000,
    0b1110000000000000,
    0b1111000000000000,
    0b1111100000000000,
    0b1111110000000000,
    0b1111111000000000,
    0b1111111100000000,
    0b1111111000000000,
    0b1111110000000000,
    0b1111000000000000,
    0b1110000000000000,
    0b1100000000000000,
    0b1100000000000000,
    0b1000000000000000,
    0b0000000000000000,
];

const COLOR_TITLE_BAR: Rgba = Rgba::new(0x00, 0x3c, 0x45, 0x55);
const COLOR_TITLE: Rgba = Rgba::new(0x00, 0xf2, 0xf4, 0xf8);
const COLOR_WINDOW_BG: Rgba = Rgba::new(0x00, 0xf8, 0xfb, 0xff);
const COLOR_BORDER: Rgba = Rgba::new(0x00, 0x25, 0x2d, 0x3a);
const COLOR_TEXT: Rgba = Rgba::new(0x00, 0x27, 0x2f, 0x3a);
const COLOR_CURSOR_PRIMARY: Rgba = Rgba::new(0x00, 0xff, 0xff, 0xff);
const COLOR_CURSOR_SHADOW: Rgba = Rgba::new(0x00, 0x00, 0x00, 0x00);
const COLOR_SHADOW: Rgba = Rgba::new(0x00, 0x0d, 0x11, 0x18);
const CLEAR_COLOR: Rgba = Rgba::new(0xff, 0x00, 0x00, 0x00);

const MAX_BACKBUFFER_PIXELS: usize = 8_388_608; // 8 Mi pixels (~32 MiB)
const SAFE_FB_WIDTH: usize = 1024;
const SAFE_FB_HEIGHT: usize = 768;

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
pub enum DrawCommand {
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
        primary: Rgba,
        shadow: Rgba,
        pressed: bool,
    },
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub width: u32,
    pub height: u32,
    commands: Vec<DrawCommand>,
}

impl Scene {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            commands: Vec::new(),
        }
    }

    pub fn push(&mut self, cmd: DrawCommand) {
        self.commands.push(cmd);
    }

    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }
}

pub enum CompositorExport<'a> {
    Framebuffer {
        addr: *const u32,
        width: usize,
        height: usize,
        stride_bytes: usize,
    },
    Svg {
        xml: &'a str,
        width: u32,
        height: u32,
    },
}

pub trait CompositorBackend {
    fn size(&self) -> (usize, usize);
    fn render(&mut self, scene: &Scene);
    fn export(&self) -> Option<CompositorExport<'_>> {
        None
    }
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

struct CursorState {
    x: i32,
    y: i32,
    buttons: u8,
    visible: bool,
}

impl CursorState {
    fn new(width: usize, height: usize) -> Self {
        Self {
            x: (width / 2) as i32,
            y: (height / 2) as i32,
            buttons: 0,
            visible: true,
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
}

pub struct Compositor<B: CompositorBackend> {
    frame_no: u64,
    backend: B,
    windows: BTreeMap<Uuid, WindowSurface>,
    window_order: Vec<Uuid>,
    watch_surfaces: Option<WatchId>,
    watch_windows: Option<WatchId>,
    watch_mouse: Option<WatchId>,
    watch_cursor: Option<WatchId>,
    watch_fb: Option<WatchId>,
    fb_id: Option<Uuid>,
    cursor: CursorState,
    background: Arc<Bitmap>,
}

impl<B: CompositorBackend> Compositor<B> {
    pub fn new(backend: B) -> Self {
        let (width, height) = backend.size();
        let background = load_background();

        Self {
            frame_no: 0,
            backend,
            windows: BTreeMap::new(),
            window_order: Vec::new(),
            watch_surfaces: None,
            watch_windows: None,
            watch_mouse: None,
            watch_cursor: None,
            watch_fb: None,
            fb_id: None,
            cursor: CursorState::new(width, height),
            background,
        }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn init_with_watches(watch_manager: &mut WatchManager, app_id: usize, backend: B) -> Self {
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

        let mut comp = Self::new(backend);
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
                    }
                }
            }
            AppEvent::Edge { .. } => {}
        }
    }

    pub fn tick(&mut self) {
        let (width, height) = self.backend.size();
        if width == 0 || height == 0 {
            return;
        }

        let mut scene = Scene::new(width as u32, height as u32);
        scene.push(DrawCommand::Clear { color: CLEAR_COLOR });
        self.draw_background(&mut scene, width, height);
        self.draw_windows(&mut scene, width, height);
        self.draw_cursor(&mut scene, width, height);

        self.backend.render(&scene);
        self.publish_frame_info();
        self.frame_no = self.frame_no.wrapping_add(1);
    }

    fn publish_frame_info(&mut self) {
        if let Some(fb_id) = self.fb_id {
            if let Some(CompositorExport::Framebuffer {
                addr,
                width,
                height,
                stride_bytes,
            }) = self.backend.export()
            {
                let mut fields = BTreeMap::new();
                fields.insert(canon::KIND, Value::Symbol(canon::DISPLAY_FRAME));
                fields.insert(canon::SEQ, Value::U64(self.frame_no));
                fields.insert(canon::ADDR, Value::U64(addr as u64));
                fields.insert(canon::WIDTH, Value::U64(width as u64));
                fields.insert(canon::HEIGHT, Value::U64(height as u64));
                fields.insert(canon::PITCH, Value::U64(stride_bytes as u64));
                fields.insert(canon::BPP, Value::U64(32));

                let frame_id = userland::fiat(None, canon::DISPLAY_FRAME, fields);
                userland::that(fb_id, canon::CURRENT_FRAME, frame_id, 0);
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

                let (width, height) = self.backend.size();
                self.cursor.update(dx, dy, buttons as u8, width, height);
            }
        }
    }

    fn ingest_cursor(&mut self, thing: &userland::GraphThing) {
        if thing.kind != canon::CURSOR {
            return;
        }
        let x = thing.fields.get(&canon::X).and_then(|v| v.as_i64());
        let y = thing.fields.get(&canon::Y).and_then(|v| v.as_i64());
        let visible = thing.fields.get(&canon::VISIBLE).and_then(|v| v.as_bool());
        let (width, height) = self.backend.size();
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
        self.bump_window(window_id);
    }

    fn bump_window(&mut self, window_id: Uuid) {
        if let Some(pos) = self.window_order.iter().position(|w| *w == window_id) {
            self.window_order.remove(pos);
        }
        self.window_order.push(window_id);
    }

    fn draw_background(&self, scene: &mut Scene, width: usize, height: usize) {
        scene.push(DrawCommand::BlitImage {
            rect: Rect::new(0, 0, width as u32, height as u32),
            image: self.background.clone(),
            repeat: true,
        });
    }

    fn draw_windows(&self, scene: &mut Scene, fb_width: usize, fb_height: usize) {
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

        for id in ordered {
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

        let shadow_offset = 3;
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(
                (x + shadow_offset) as i32,
                (y + shadow_offset) as i32,
                w as u32,
                h as u32,
            ),
            color: COLOR_SHADOW,
        });

        scene.push(DrawCommand::FillRect {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
            color: COLOR_BORDER,
        });
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(
                (x + BORDER_THICKNESS) as i32,
                (y + BORDER_THICKNESS) as i32,
                w.saturating_sub(BORDER_THICKNESS * 2) as u32,
                h.saturating_sub(BORDER_THICKNESS * 2) as u32,
            ),
            color: COLOR_WINDOW_BG,
        });

        scene.push(DrawCommand::FillRect {
            rect: Rect::new(
                (x + BORDER_THICKNESS) as i32,
                (y + BORDER_THICKNESS) as i32,
                w.saturating_sub(BORDER_THICKNESS * 2) as u32,
                TITLE_BAR_HEIGHT as u32,
            ),
            color: COLOR_TITLE_BAR,
        });
        scene.push(DrawCommand::DrawText {
            origin: (
                (x + BORDER_THICKNESS + WINDOW_PADDING) as i32,
                (y + BORDER_THICKNESS + 2) as i32,
            ),
            text: surface.window.title.clone(),
            color: COLOR_TITLE,
            max_width: Some(w.saturating_sub((BORDER_THICKNESS + WINDOW_PADDING) * 2) as u32),
        });

        let client_x = x + BORDER_THICKNESS + WINDOW_PADDING;
        let client_y = y + BORDER_THICKNESS + TITLE_BAR_HEIGHT + WINDOW_PADDING;
        let client_w = w.saturating_sub(BORDER_THICKNESS * 2 + WINDOW_PADDING * 2);
        let client_h =
            h.saturating_sub(TITLE_BAR_HEIGHT + BORDER_THICKNESS * 2 + WINDOW_PADDING * 2);

        if let Some(bmp) = &surface.bitmap {
            scene.push(DrawCommand::BlitImage {
                rect: Rect::new(
                    client_x as i32,
                    client_y as i32,
                    client_w as u32,
                    client_h as u32,
                ),
                image: bmp.clone(),
                repeat: true,
            });
        }

        scene.push(DrawCommand::DrawTextBlock {
            rect: Rect::new(
                client_x as i32,
                client_y as i32,
                client_w as u32,
                client_h as u32,
            ),
            text: surface.text.clone(),
            color: COLOR_TEXT,
        });
    }

    fn draw_cursor(&self, scene: &mut Scene, fb_width: usize, fb_height: usize) {
        if !self.cursor.visible {
            return;
        }
        let base_x = clamp_i32(self.cursor.x, 0, fb_width.saturating_sub(1) as i32) as i32;
        let base_y = clamp_i32(self.cursor.y, 0, fb_height.saturating_sub(1) as i32) as i32;

        scene.push(DrawCommand::DrawCursor {
            origin: (base_x, base_y),
            primary: if self.cursor.buttons & 0x1 != 0 {
                COLOR_TITLE_BAR
            } else {
                COLOR_CURSOR_PRIMARY
            },
            shadow: COLOR_CURSOR_SHADOW,
            pressed: self.cursor.buttons & 0x1 != 0,
        });
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
    }
}

fn clamp_i32(v: i32, min_v: i32, max_v: i32) -> i32 {
    max(min_v, min(v, max_v))
}

fn sanitize_fb_info(info: FramebufferInfo) -> FramebufferInfo {
    const MAX_DIM: usize = 4096;
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
    FramebufferInfo {
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
pub struct FramebufferInfo {
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub bpp: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct FramebufferTarget {
    pub info: FramebufferInfo,
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
        scene.push(DrawCommand::Clear { color: CLEAR_COLOR });
        scene.push(DrawCommand::BlitImage {
            rect: Rect::new(0, 0, 10, 10),
            image: bitmap,
            repeat: true,
        });
        assert_eq!(scene.commands().len(), 2);
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
        let mut backend = SvgBackend::new(10, 10);
        let mut scene = Scene::new(10, 10);
        scene.push(DrawCommand::Clear {
            color: Rgba::opaque(0, 0, 0),
        });
        backend.render(&scene);
        let xml = match backend.export() {
            Some(CompositorExport::Svg { xml, .. }) => xml.to_string(),
            _ => String::new(),
        };
        assert!(xml.contains("<svg"));
    }

    #[cfg(feature = "host")]
    #[test]
    fn color_layout_matches_old_values() {
        assert_eq!(COLOR_TITLE_BAR.to_u32(), 0x003c4555);
        assert_eq!(CLEAR_COLOR.to_u32(), 0xff000000);
    }
}
