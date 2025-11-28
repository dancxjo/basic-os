use alloc::sync::Arc;
use alloc::{collections::BTreeMap, string::ToString};
use core::cmp::{max, min};
use core::ops::Range;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use limine::request::FramebufferRequest;
use spin::Mutex as SpinMutex;
use x86_64::VirtAddr;

use crate::drivers::device::{self, DeviceKind};
use crate::telemetry::canon;
use crate::telemetry::graph::{self, GraphFiatRequest};
use crate::telemetry::journal::Value;
use uuid::Uuid;

static FRAMEBUFFER_VIRT_RANGE: SpinMutex<Option<Range<VirtAddr>>> = SpinMutex::new(None);
static FRAMEBUFFER_REGION: SpinMutex<Option<(u64, usize)>> = SpinMutex::new(None);
static FRAMEBUFFER_DEVICE: SpinMutex<Option<Arc<SpinMutex<Framebuffer>>>> = SpinMutex::new(None);

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FramebufferInfo {
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u64,
    pub addr: u64,
}

static FRAMEBUFFER_INFO: SpinMutex<Option<FramebufferInfo>> = SpinMutex::new(None);

pub fn get_framebuffer_info() -> Option<FramebufferInfo> {
    *FRAMEBUFFER_INFO.lock()
}

pub fn get_framebuffer_virt_range() -> Option<Range<VirtAddr>> {
    FRAMEBUFFER_VIRT_RANGE.lock().clone()
}

fn set_framebuffer_region(base: u64, len: usize) {
    *FRAMEBUFFER_REGION.lock() = Some((base, len));
}

#[derive(Debug)]
pub struct Framebuffer {
    fb: &'static mut [u32],
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub pitch_pixels: usize,
    pub bpp: u16,
}

impl Framebuffer {
    pub fn new() -> Option<Self> {
        static FB_REQUEST: FramebufferRequest = FramebufferRequest::new();
        let response = FB_REQUEST.get_response()?;
        let fb_info = response.framebuffers().next()?;

        let width = fb_info.width() as usize;
        let height = fb_info.height() as usize;
        let pitch = fb_info.pitch() as usize;
        let pitch_pixels = pitch / 4;
        let len = pitch_pixels * height;
        let virt_addr = fb_info.addr();

        let info = FramebufferInfo {
            width: width as u64,
            height: height as u64,
            pitch: pitch as u64,
            bpp: fb_info.bpp() as u64,
            addr: virt_addr as u64,
        };
        *FRAMEBUFFER_INFO.lock() = Some(info);

        *FRAMEBUFFER_VIRT_RANGE.lock() = Some(
            VirtAddr::new(virt_addr as u64)..VirtAddr::new((virt_addr as u64) + (len * 4) as u64),
        );

        let fb_ptr = virt_addr as *mut u32;
        let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb_ptr, len) };
        set_framebuffer_region(virt_addr as u64, len * core::mem::size_of::<u32>());

        Some(Self {
            fb: fb_slice,
            width,
            height,
            pitch,
            pitch_pixels,
            bpp: fb_info.bpp(),
        })
    }

    pub fn fb_len(&self) -> usize {
        self.fb.len()
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn pitch_pixels(&self) -> usize {
        self.pitch_pixels
    }

    pub fn blit(&mut self, x: usize, y: usize, w: usize, h: usize, data: &[u32]) -> Result<(), ()> {
        if x + w > self.width || y + h > self.height {
            return Err(());
        }

        let start = y * self.pitch_pixels + x;
        for row in 0..h {
            let dest =
                &mut self.fb[start + row * self.pitch_pixels..start + row * self.pitch_pixels + w];
            dest.copy_from_slice(&data[row * w..row * w + w]);
        }
        Ok(())
    }

    pub fn clear(&mut self, color: u32) {
        self.fb.fill(color);
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let idx = y * self.pitch_pixels + x;
            self.fb[idx] = color;
        }
    }
}

pub struct FramebufferConsole {
    framebuffer: Arc<SpinMutex<Framebuffer>>,
    cursor_x: usize,
    cursor_y: usize,
    columns: usize,
    rows: usize,
    fg_color: u32,
    bg_color: u32,
}

static CONSOLE: SpinMutex<Option<FramebufferConsole>> = SpinMutex::new(None);

impl FramebufferConsole {
    const CHAR_WIDTH: usize = 8;
    const CHAR_HEIGHT: usize = 8;

    pub fn new(framebuffer: Arc<SpinMutex<Framebuffer>>) -> Self {
        let (columns, rows) = {
            let fb = framebuffer.lock();
            (
                max(1, fb.width / Self::CHAR_WIDTH),
                max(1, fb.height / Self::CHAR_HEIGHT),
            )
        };

        let console = Self {
            framebuffer: framebuffer.clone(),
            cursor_x: 0,
            cursor_y: 0,
            columns,
            rows,
            fg_color: 0x00FFFFFF,
            bg_color: 0x00000000,
        };

        {
            let mut fb = framebuffer.lock();
            console.clear_framebuffer(&mut fb);
        }

        console
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.carriage_return(),
            b'\t' => {
                for _ in 0..4 {
                    self.write_byte(b' ');
                }
            }
            _ => self.put_visible(byte),
        }
    }

    fn clear_framebuffer(&self, fb: &mut Framebuffer) {
        fb.clear(self.bg_color);
    }

    fn put_visible(&mut self, byte: u8) {
        let glyph = BASIC_FONTS
            .get(byte as char)
            .or_else(|| BASIC_FONTS.get('?'))
            .unwrap_or([0; 8]);

        let x = self.cursor_x * Self::CHAR_WIDTH;
        let y = self.cursor_y * Self::CHAR_HEIGHT;

        {
            let mut fb = self.framebuffer.lock();
            self.render_glyph(&mut fb, &glyph, x, y);
        }
        self.advance_cursor();
    }

    fn render_glyph(
        &self,
        fb: &mut Framebuffer,
        glyph: &[u8; 8],
        origin_x: usize,
        origin_y: usize,
    ) {
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..Self::CHAR_WIDTH {
                let color = if bits & (1 << col) != 0 {
                    self.fg_color
                } else {
                    self.bg_color
                };
                fb.set_pixel(origin_x + col, origin_y + row, color);
            }
        }
    }

    fn advance_cursor(&mut self) {
        self.cursor_x += 1;
        if self.cursor_x >= self.columns {
            self.cursor_x = 0;
            self.cursor_y += 1;
        }

        if self.cursor_y >= self.rows {
            let mut fb = self.framebuffer.lock();
            self.scroll_framebuffer(&mut fb);
            self.cursor_y = self.rows - 1;
        }
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        self.cursor_y += 1;
        let mut fb = self.framebuffer.lock();
        if self.cursor_y >= self.rows {
            self.scroll_framebuffer(&mut fb);
            self.cursor_y = self.rows - 1;
        }
    }

    fn carriage_return(&mut self) {
        self.cursor_x = 0;
    }

    fn scroll_framebuffer(&self, fb: &mut Framebuffer) {
        if fb.height < Self::CHAR_HEIGHT {
            return;
        }

        let stride = fb.pitch_pixels;
        let copy_rows = fb.height - Self::CHAR_HEIGHT;
        for row in 0..copy_rows {
            let dst = row * stride;
            let src = (row + Self::CHAR_HEIGHT) * stride;
            fb.fb.copy_within(src..src + stride, dst);
        }

        let clear_start = copy_rows * stride;
        let clear_len = Self::CHAR_HEIGHT * stride;
        fb.fb[clear_start..clear_start + clear_len].fill(self.bg_color);
    }
}

pub fn init_console(framebuffer: Arc<SpinMutex<Framebuffer>>) {
    let mut console = CONSOLE.lock();
    *console = Some(FramebufferConsole::new(framebuffer));
}

pub fn console_write_byte(byte: u8) {
    if let Some(ref mut console) = *CONSOLE.lock() {
        console.write_byte(byte);
    }
}

fn map_framebuffer() -> Option<(u64, usize)> {
    FRAMEBUFFER_REGION.lock().clone()
}

fn write_framebuffer(buf: &[u8]) -> usize {
    let fb_arc = match FRAMEBUFFER_DEVICE.lock().clone() {
        Some(fb) => fb,
        None => return 0,
    };
    let mut fb = fb_arc.lock();
    let dst = unsafe {
        core::slice::from_raw_parts_mut(
            fb.fb.as_mut_ptr() as *mut u8,
            fb.fb_len() * core::mem::size_of::<u32>(),
        )
    };
    let count = min(buf.len(), dst.len());
    if count > 0 {
        dst[..count].copy_from_slice(&buf[..count]);
    }
    count
}

fn read_framebuffer(buf: &mut [u8]) -> usize {
    let fb_arc = match FRAMEBUFFER_DEVICE.lock().clone() {
        Some(fb) => fb,
        None => return 0,
    };
    let fb = fb_arc.lock();
    let required = core::mem::size_of::<u32>() * 4;
    if buf.len() < required {
        return 0;
    }

    let width = (fb.width as u32).to_le_bytes();
    let height = (fb.height as u32).to_le_bytes();
    let pitch = (fb.pitch as u32).to_le_bytes();
    let bpp = (fb.bpp as u32).to_le_bytes();

    buf[..4].copy_from_slice(&width);
    buf[4..8].copy_from_slice(&height);
    buf[8..12].copy_from_slice(&pitch);
    buf[12..16].copy_from_slice(&bpp);
    required
}

/// Advertise the framebuffer as a device endpoint so userland drivers can map
/// or push pixel data directly.
pub fn register_framebuffer_device(framebuffer: Arc<SpinMutex<Framebuffer>>) {
    let fb = framebuffer.lock();
    let mut fields = BTreeMap::new();
    fields.insert(canon::WIDTH, Value::U64(fb.width as u64));
    fields.insert(canon::HEIGHT, Value::U64(fb.height as u64));
    fields.insert(canon::PITCH, Value::U64(fb.pitch as u64));
    fields.insert(canon::BPP, Value::U64(fb.bpp as u64));
    if let Some((addr, _len)) = *FRAMEBUFFER_REGION.lock() {
        fields.insert(canon::ADDR, Value::U64(addr));
    }
    let node = device::create_device_node(
        canon::FRAMEBUFFER_DEVICE,
        device::FRAMEBUFFER_DEVICE_NAME,
        fields,
    );
    drop(fb);
    *FRAMEBUFFER_DEVICE.lock() = Some(framebuffer);
    device::register_device(
        DeviceKind::Framebuffer,
        Some(read_framebuffer),
        Some(write_framebuffer),
        Some(map_framebuffer),
        Some(node),
    );
}

/// Publish framebuffer metadata into the shared graph so userland can discover
/// the display surface without bespoke syscalls.
pub fn publish_framebuffer_node(framebuffer: Arc<SpinMutex<Framebuffer>>) {
    let fb = framebuffer.lock();
    let mut fields = BTreeMap::new();
    fields.insert(canon::NAME, Value::Text("framebuffer0".to_string()));
    fields.insert(canon::STATUS, Value::Symbol(canon::INIT));
    fields.insert(canon::WIDTH, Value::U64(fb.width as u64));
    fields.insert(canon::HEIGHT, Value::U64(fb.height as u64));
    fields.insert(canon::PITCH, Value::U64(fb.pitch as u64));
    fields.insert(canon::BPP, Value::U64(fb.bpp as u64));
    if let Some((addr, _len)) = *FRAMEBUFFER_REGION.lock() {
        fields.insert(canon::ADDR, Value::U64(addr));
    }

    let framebuffer_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"framebuffer0");
    let req = GraphFiatRequest {
        id: Some(framebuffer_id),
        kind: canon::PIXMAP,
        fields,
    };
    let _ = graph::fiat_for_bundle(graph::KERNEL_BUNDLE_ID, req);
}
