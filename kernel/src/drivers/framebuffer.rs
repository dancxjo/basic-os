use alloc::sync::Arc;
use core::cmp::max;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use limine::request::FramebufferRequest;
use spin::Mutex as SpinMutex;

use crate::bootloader::get_hhdm_offset;
use crate::drivers::registry::{DriverDescriptor, DriverKind};

const MAX_WIDTH: usize = 3840;
const MAX_HEIGHT: usize = 2160;

#[derive(Debug)]
pub struct Framebuffer {
    fb: &'static mut [u32],
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub pitch_pixels: usize,
    pub bpp: u16,
}

pub const DRIVER: DriverDescriptor = DriverDescriptor::new(
    "limine-framebuffer",
    DriverKind::Display,
    "Framebuffer provided by Limine bootloader",
    init_driver,
);

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
        let phys_addr = fb_info.addr() as u64;
        let hhdm = get_hhdm_offset().as_u64();
        let virt_addr = if phys_addr >= hhdm {
            phys_addr
        } else {
            hhdm.checked_add(phys_addr)
                .expect("Framebuffer address overflowed HHDM computation")
        };
        let fb_ptr = virt_addr as *mut u32;
        let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb_ptr, len) };

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

/// Lightweight probe hook so the driver registry can announce presence.
pub fn init_driver() -> Result<(), &'static str> {
    Framebuffer::new()
        .map(|_| ())
        .ok_or("Framebuffer not available")
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
