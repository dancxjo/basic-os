#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};
use core::convert::TryInto;
use core::ptr;

use unifont::{get_glyph, Glyph};
use userland::{
    canon, extract_text, load_thing, println, AppEvent, Symbol, ThingFilter, Thingable, Value,
    WatchId, WatchManager, Window,
};
use uuid::Uuid;

const FONT_HEIGHT: usize = 16;
const TITLE_BAR_HEIGHT: usize = FONT_HEIGHT + 4;
const BORDER_THICKNESS: usize = 2;
const WINDOW_PADDING: usize = 6;
const CURSOR_SIZE: usize = 16;

const COLOR_TITLE_BAR: u32 = 0x003c4555;
const COLOR_TITLE: u32 = 0x00f2f4f8;
const COLOR_WINDOW_BG: u32 = 0x00f8fbff;
const COLOR_BORDER: u32 = 0x00252d3a;
const COLOR_TEXT: u32 = 0x00272f3a;
const COLOR_CURSOR_PRIMARY: u32 = 0x00ffffff;
const COLOR_CURSOR_SHADOW: u32 = 0x00000000;
const MAX_BACKBUFFER_PIXELS: usize = 8_388_608; // 8 Mi pixels (~32 MiB)
const SAFE_FB_WIDTH: usize = 1024;
const SAFE_FB_HEIGHT: usize = 768;

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

pub struct Compositor {
    frame_no: u64,
    framebuffer: FramebufferSurface,
    backbuffer: Vec<u32>,
    windows: BTreeMap<Uuid, WindowSurface>,
    window_order: Vec<Uuid>,
    watch_window_buffers: Option<WatchId>,
    watch_windows: Option<WatchId>,
    watch_mouse: Option<WatchId>,
    watch_fb: Option<WatchId>,
    fb_id: Option<Uuid>,
    cursor: CursorState,
    background: Bitmap,
}

struct FramebufferSurface {
    width: usize,
    height: usize,
    stride: usize,
}

#[derive(Clone)]
struct WindowSurface {
    window: Window,
    pixmap: Uuid,
    text: String,
}

struct CursorState {
    x: i32,
    y: i32,
    buttons: u8,
}

impl CursorState {
    fn new(width: usize, height: usize) -> Self {
        Self {
            x: (width / 2) as i32,
            y: (height / 2) as i32,
            buttons: 0,
        }
    }

    fn update(&mut self, dx: i64, dy: i64, buttons: u8, width: usize, height: usize) {
        self.x = clamp_i32(self.x + dx as i32, 0, width.saturating_sub(1) as i32);
        self.y = clamp_i32(self.y + dy as i32, 0, height.saturating_sub(1) as i32);
        self.buttons = buttons;
    }
}

struct Bitmap {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}

impl Bitmap {
    fn sample(&self, x: usize, y: usize) -> u32 {
        if self.width == 0 || self.height == 0 {
            return 0;
        }
        let sx = x % self.width;
        let sy = y % self.height;
        self.pixels[sy * self.width + sx]
    }
}

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

impl Compositor {
    pub fn new(target: FramebufferTarget) -> Self {
        let fb_info = sanitize_fb_info(target.info);
        let mut stride = max(fb_info.pitch / 4, fb_info.width.max(1));
        let mut height = fb_info.height.max(1);
        if stride.saturating_mul(height) > MAX_BACKBUFFER_PIXELS {
            println!(
                "Clamping framebuffer from {}x{} stride {} to {}x{}",
                fb_info.width, fb_info.height, stride, SAFE_FB_WIDTH, SAFE_FB_HEIGHT
            );
            stride = SAFE_FB_WIDTH;
            height = SAFE_FB_HEIGHT;
        } else if stride.saturating_mul(height) > 2_000_000 {
            stride = max(SAFE_FB_WIDTH, fb_info.width);
            height = max(SAFE_FB_HEIGHT, fb_info.height);
        }
        let width = min(fb_info.width.max(1), stride);
        let background = load_background();

        Self {
            frame_no: 0,
            framebuffer: FramebufferSurface {
                width,
                height,
                stride,
            },
            backbuffer: vec![0u32; stride * height],
            windows: BTreeMap::new(),
            window_order: Vec::new(),
            watch_window_buffers: None,
            watch_windows: None,
            watch_mouse: None,
            watch_fb: None,
            fb_id: None,
            cursor: CursorState::new(fb_info.width, height),
            background,
        }
    }

    pub fn init_with_watches(
        watch_manager: &mut WatchManager,
        app_id: usize,
        fb: FramebufferTarget,
    ) -> Self {
        let window_buffer_watch = watch_manager.register_graph(
            app_id,
            ThingFilter {
                kind: Some(canon::WINDOW_BUFFER_UPDATED),
                id: None,
            },
        );
        let mouse_watch = watch_manager.register_graph(
            app_id,
            ThingFilter {
                kind: Some(canon::INPUT_EVENT),
                id: None,
            },
        );
        let window_watch = watch_manager.register_graph(
            app_id,
            ThingFilter {
                kind: Some(canon::WINDOW),
                id: None,
            },
        );
        let fb_watch = watch_manager.register_graph(
            app_id,
            ThingFilter {
                kind: Some(canon::DISPLAY_FRAMEBUFFER),
                id: None,
            },
        );

        let mut comp = Self::new(fb);
        comp.watch_window_buffers = Some(window_buffer_watch);
        comp.watch_windows = Some(window_watch);
        comp.watch_mouse = Some(mouse_watch);
        comp.watch_fb = Some(fb_watch);

        // Initial window discovery
        let windows = userland::graph::find_by_kind("window");
        for thing in windows {
            if let Some(window) = Window::load(&thing) {
                comp.ingest_window(window);
            }
        }

        comp
    }

    pub fn on_event(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::Thing { watch, thing } => {
                if Some(*watch) == self.watch_window_buffers {
                    self.ingest_window_buffer(thing);
                } else if Some(*watch) == self.watch_mouse {
                    if thing.kind == canon::INPUT_EVENT {
                        self.ingest_input_event(thing);
                    }
                } else if Some(*watch) == self.watch_windows {
                    if let Some(window) = Window::load(thing) {
                        self.ingest_window(window);
                    }
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
        if self.backbuffer.is_empty() || self.framebuffer.width == 0 || self.framebuffer.height == 0
        {
            return;
        }
        let needed = self
            .framebuffer
            .stride
            .saturating_mul(self.framebuffer.height);
        if self.backbuffer.len() < needed {
            self.backbuffer.resize(needed, 0);
        }

        // Clear backbuffer
        self.backbuffer.fill(0xFF000000);

        self.draw_background();
        self.draw_windows();
        self.draw_cursor();
        self.present();
        self.frame_no = self.frame_no.wrapping_add(1);
    }

    fn ingest_window_buffer(&mut self, thing: &userland::GraphThing) {
        if thing.kind != canon::WINDOW_BUFFER_UPDATED {
            return;
        }
        let map = &thing.fields;
        let window_id = map.get(&canon::SRC).and_then(Value::as_uuid);
        let pixmap = map.get(&canon::TARGET).and_then(Value::as_uuid);
        let text = map
            .get(&canon::TEXT)
            .and_then(extract_text)
            .unwrap_or_default();
        let Some(window_id) = window_id else {
            return;
        };
        let pixmap = pixmap.unwrap_or_else(|| Uuid::nil());

        let window = load_thing::<Window>(window_id).unwrap_or_else(|| default_window(window_id));
        let entry = self.windows.entry(window_id).or_insert(WindowSurface {
            window: window.clone(),
            pixmap,
            text: String::new(),
        });
        entry.window = window;
        entry.pixmap = pixmap;
        entry.text = text;
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

                self.cursor.update(
                    dx,
                    dy,
                    buttons as u8,
                    self.framebuffer.width,
                    self.framebuffer.height,
                );
            }
        }
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
                    pixmap: Uuid::nil(),
                    text: String::new(),
                },
            );
        }
        self.bump_window(window_id);
    }

    fn bump_window(&mut self, window_id: Uuid) {
        if let Some(pos) = self.window_order.iter().position(|w| *w == window_id) {
            self.window_order.remove(pos);
        }
        self.window_order.push(window_id);
    }

    fn draw_background(&mut self) {
        for y in 0..self.framebuffer.height {
            let row_offset = y * self.framebuffer.stride;
            for x in 0..self.framebuffer.stride {
                let color = self.background.sample(x, y);
                self.backbuffer[row_offset + x] = color;
            }
        }
    }

    fn draw_windows(&mut self) {
        let ordered: Vec<WindowSurface> = self
            .window_order
            .iter()
            .filter_map(|id| self.windows.get(id).cloned())
            .collect();
        for surface in ordered {
            self.draw_window(&surface);
        }
    }

    fn draw_window(&mut self, surface: &WindowSurface) {
        let w = surface.window.width as usize;
        let h = surface.window.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        let x = min(surface.window.x as usize, self.framebuffer.width);
        let y = min(surface.window.y as usize, self.framebuffer.height);

        let shadow_offset = 3;
        self.fill_rect(x + shadow_offset, y + shadow_offset, w, h, 0x000d1118);

        self.draw_border(x, y, w, h, COLOR_BORDER);
        self.fill_rect(
            x + BORDER_THICKNESS,
            y + BORDER_THICKNESS,
            w.saturating_sub(BORDER_THICKNESS * 2),
            h.saturating_sub(BORDER_THICKNESS * 2),
            COLOR_WINDOW_BG,
        );

        self.fill_rect(
            x + BORDER_THICKNESS,
            y + BORDER_THICKNESS,
            w.saturating_sub(BORDER_THICKNESS * 2),
            TITLE_BAR_HEIGHT,
            COLOR_TITLE_BAR,
        );
        self.draw_text(
            x + BORDER_THICKNESS + WINDOW_PADDING,
            y + BORDER_THICKNESS + 2,
            surface.window.title.as_str(),
            COLOR_TITLE,
            Some(w.saturating_sub((BORDER_THICKNESS + WINDOW_PADDING) * 2)),
        );

        let client_x = x + BORDER_THICKNESS + WINDOW_PADDING;
        let client_y = y + BORDER_THICKNESS + TITLE_BAR_HEIGHT + WINDOW_PADDING;
        let client_w = w.saturating_sub(BORDER_THICKNESS * 2 + WINDOW_PADDING * 2);
        let client_h =
            h.saturating_sub(TITLE_BAR_HEIGHT + BORDER_THICKNESS * 2 + WINDOW_PADDING * 2);

        self.draw_text_block(
            client_x,
            client_y,
            client_w,
            client_h,
            surface.text.as_str(),
            COLOR_TEXT,
        );
    }

    fn draw_border(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        self.fill_rect(x, y, w, BORDER_THICKNESS, color);
        self.fill_rect(
            x,
            y + h.saturating_sub(BORDER_THICKNESS),
            w,
            BORDER_THICKNESS,
            color,
        );
        self.fill_rect(x, y, BORDER_THICKNESS, h, color);
        self.fill_rect(
            x + w.saturating_sub(BORDER_THICKNESS),
            y,
            BORDER_THICKNESS,
            h,
            color,
        );
    }

    fn draw_text(&mut self, x: usize, y: usize, text: &str, color: u32, max_width: Option<usize>) {
        let mut cursor_x = x;
        let mut cursor_y = y;
        for ch in text.chars() {
            if ch == '\n' {
                cursor_x = x;
                cursor_y += FONT_HEIGHT;
                continue;
            }
            let Some(glyph) = get_glyph(ch) else { continue };
            let gw = glyph.get_width();
            if let Some(limit) = max_width {
                if cursor_x + gw > x + limit {
                    break;
                }
            }
            self.draw_glyph(cursor_x, cursor_y, glyph, color);
            cursor_x += gw;
        }
    }

    fn draw_text_block(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        text: &str,
        color: u32,
    ) {
        let mut cursor_x = x;
        let mut cursor_y = y;
        for ch in text.chars() {
            if ch == '\n' {
                cursor_x = x;
                cursor_y += FONT_HEIGHT;
                if cursor_y + FONT_HEIGHT >= y + height {
                    break;
                }
                continue;
            }
            let Some(glyph) = get_glyph(ch) else { continue };
            let gw = glyph.get_width();
            if cursor_x + gw >= x + width {
                cursor_x = x;
                cursor_y += FONT_HEIGHT;
                if cursor_y + FONT_HEIGHT >= y + height {
                    break;
                }
            }
            self.draw_glyph(cursor_x, cursor_y, glyph, color);
            cursor_x += gw;
        }
    }

    fn draw_glyph(&mut self, x: usize, y: usize, glyph: &Glyph, color: u32) {
        if x >= self.framebuffer.width || y >= self.framebuffer.height {
            return;
        }

        let width = glyph.get_width();
        for row in 0..FONT_HEIGHT {
            let dst_y = y + row;
            if dst_y >= self.framebuffer.height {
                break;
            }
            for col in 0..width {
                let dst_x = x + col;
                if dst_x >= self.framebuffer.width {
                    break;
                }
                if glyph.get_pixel(col, row) {
                    let idx = dst_y * self.framebuffer.stride + dst_x;
                    self.backbuffer[idx] = color;
                }
            }
        }
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let x0 = min(x, self.framebuffer.width);
        let y0 = min(y, self.framebuffer.height);
        let x1 = min(x0 + w, self.framebuffer.width);
        let y1 = min(y0 + h, self.framebuffer.height);
        for yy in y0..y1 {
            let row = yy * self.framebuffer.stride;
            for xx in x0..x1 {
                self.backbuffer[row + xx] = color;
            }
        }
    }

    fn draw_cursor(&mut self) {
        let base_x = clamp_i32(
            self.cursor.x,
            0,
            self.framebuffer.width.saturating_sub(1) as i32,
        ) as usize;
        let base_y = clamp_i32(
            self.cursor.y,
            0,
            self.framebuffer.height.saturating_sub(1) as i32,
        ) as usize;

        for (row, mask) in CURSOR_MASK.iter().enumerate() {
            let y = base_y + row;
            if y >= self.framebuffer.height {
                break;
            }
            for col in 0..CURSOR_SIZE {
                let bit = 15 - col;
                if (mask & (1 << bit)) == 0 {
                    continue;
                }

                let x = base_x + col;
                if x >= self.framebuffer.width {
                    break;
                }

                if x + 1 < self.framebuffer.width && y + 1 < self.framebuffer.height {
                    let shadow_idx = (y + 1) * self.framebuffer.stride + (x + 1);
                    self.backbuffer[shadow_idx] = COLOR_CURSOR_SHADOW;
                }

                let idx = y * self.framebuffer.stride + x;
                self.backbuffer[idx] = if self.cursor.buttons & 0x1 != 0 {
                    COLOR_TITLE_BAR
                } else {
                    COLOR_CURSOR_PRIMARY
                };
            }
        }
    }

    fn present(&self) {
        if let Some(fb_id) = self.fb_id {
            let mut fields = BTreeMap::new();
            fields.insert(canon::KIND, Value::Symbol(canon::DISPLAY_FRAME));
            fields.insert(canon::SEQ, Value::U64(self.frame_no));
            fields.insert(canon::ADDR, Value::U64(self.backbuffer.as_ptr() as u64));
            fields.insert(canon::WIDTH, Value::U64(self.framebuffer.width as u64));
            fields.insert(canon::HEIGHT, Value::U64(self.framebuffer.height as u64));
            fields.insert(
                canon::PITCH,
                Value::U64((self.framebuffer.stride * 4) as u64),
            );
            fields.insert(canon::BPP, Value::U64(32));

            let frame_id = userland::fiat(None, canon::DISPLAY_FRAME, fields);
            userland::that(fb_id, canon::CURRENT_FRAME, frame_id, 0);
        }
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

fn load_background() -> Bitmap {
    let data = include_bytes!("../../clouds.bmp");
    decode_bmp(data).unwrap_or_else(|| fallback_background())
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
    Bitmap {
        width,
        height,
        pixels,
    }
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

    Some(Bitmap {
        width: width_u,
        height: height_u,
        pixels,
    })
}
