// Framebuffer with embedded-graphics integration (adaptive RGB565 or RGB888)
use crate::bootloader::get_module;

use alloc::{collections::BTreeMap, vec::Vec};
use core::convert::Infallible;
use limine::request::FramebufferRequest;

use embedded_graphics::{
    image::Image,
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
};
use tinybmp::Bmp;

const MAX_WIDTH: usize = 3840;
const MAX_HEIGHT: usize = 2160;

#[unsafe(link_section = ".bss.uninit")]
#[unsafe(no_mangle)]
pub static mut BACKBUFFER: [u32; MAX_WIDTH * MAX_HEIGHT] = [0; MAX_WIDTH * MAX_HEIGHT];

#[derive(Clone)]
pub struct Glyph {
    pub width: usize,
    pub height: usize,
    pub bitmap: [u8; 16],
}

pub struct Framebuffer {
    fb: &'static mut [u32],
    backbuffer: &'static mut [u32],
    width: usize,
    height: usize,
    pitch: usize,
    pitch_pixels: usize,
    bpp: u16,
    glyphs: BTreeMap<u32, Glyph>,
}

impl DrawTarget for Framebuffer {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            let x = coord.x as usize;
            let y = coord.y as usize;
            if x < self.width && y < self.height {
                let index = y * self.pitch_pixels + x;
                self.backbuffer[index] = self.encode_color(color);
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl Framebuffer {
    pub fn init() -> Option<Self> {
        static FB_REQUEST: FramebufferRequest = FramebufferRequest::new();
        let response = FB_REQUEST.get_response()?;
        let fb_info = response.framebuffers().next()?;

        let width = fb_info.width() as usize;
        let height = fb_info.height() as usize;
        let pitch = fb_info.pitch() as usize;
        let pitch_pixels = pitch / 4;
        let len = pitch_pixels * height;
        let fb_ptr = fb_info.addr() as *mut u32;
        let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb_ptr, len) };
        let backbuffer = unsafe { &mut BACKBUFFER[..len] };
        let glyphs = Self::load_glyphs();

        Some(Self {
            fb: fb_slice,
            backbuffer,
            width,
            height,
            pitch,
            pitch_pixels,
            bpp: fb_info.bpp(),
            glyphs,
        })
    }

    fn encode_color(&self, color: Rgb565) -> u32 {
        match self.bpp {
            16 => {
                let raw: u16 =
                    ((color.r() as u16) << 11) | ((color.g() as u16) << 5) | (color.b() as u16);
                raw as u32
            }
            24 | 32 => {
                ((color.r() as u32) << 19) | ((color.g() as u32) << 10) | ((color.b() as u32) << 3)
            }
            _ => 0,
        }
    }

    fn load_glyphs() -> BTreeMap<u32, Glyph> {
        let mut glyphs = BTreeMap::new();
        let data = get_module("unifont.bdf").expect("Font module not loaded!");
        let mut lines = data.split(|&b| b == b'\n');
        let mut current_codepoint: Option<u32> = None;
        let mut current_bitmap: Vec<u8> = Vec::new();

        while let Some(line) = lines.next() {
            if line.starts_with(b"STARTCHAR") {
                current_bitmap.clear();
                current_codepoint = None;
            } else if line.starts_with(b"ENCODING ") {
                if let Ok(s) = core::str::from_utf8(line) {
                    if let Some(code_str) = s.strip_prefix("ENCODING ") {
                        current_codepoint = code_str.trim().parse::<u32>().ok();
                    }
                }
            } else if line.starts_with(b"BITMAP") {
                current_bitmap.clear();
                while let Some(bitmap_line) = lines.next() {
                    if bitmap_line.starts_with(b"ENDCHAR") {
                        if let Some(cp) = current_codepoint {
                            if current_bitmap.len() <= 16 && !glyphs.contains_key(&cp) {
                                let mut bitmap_arr = [0u8; 16];
                                for (i, byte) in current_bitmap.iter().enumerate() {
                                    bitmap_arr[i] = *byte;
                                }
                                glyphs.insert(
                                    cp,
                                    Glyph {
                                        width: 8,
                                        height: 16,
                                        bitmap: bitmap_arr,
                                    },
                                );
                            }
                        }
                        break;
                    }
                    if let Ok(hex_str) = core::str::from_utf8(bitmap_line) {
                        if let Ok(byte) = u8::from_str_radix(hex_str.trim(), 16) {
                            current_bitmap.push(byte);
                        }
                    }
                }
            }
        }

        glyphs
    }

    pub fn flush(&mut self) {
        self.fb.copy_from_slice(&self.backbuffer[..self.fb.len()]);
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn pitch(&self) -> usize {
        self.pitch
    }

    pub fn mono_text_style(&self, color: Rgb565) -> MonoTextStyle<'_, Rgb565> {
        MonoTextStyleBuilder::new()
            .text_color(color)
            .background_color(Rgb565::BLACK)
            .build()
    }

    pub fn clear(&mut self, color: u32) {
        for i in 0..self.pitch_pixels * self.height {
            self.backbuffer[i] = color;
        }
    }

    pub fn draw_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = y * self.pitch_pixels + x;
        self.backbuffer[offset] = color;
    }

    pub fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for dx in 0..w {
            self.draw_pixel(x + dx, y, color);
            self.draw_pixel(x + dx, y + h - 1, color);
        }
        for dy in 0..h {
            self.draw_pixel(x, y + dy, color);
            self.draw_pixel(x + w - 1, y + dy, color);
        }
    }

    pub fn draw_char(&mut self, x: usize, y: usize, ch: char, color: u32) {
        if let Some(glyph) = self.glyphs.get(&(ch as u32)) {
            if x + glyph.width > self.width || y + glyph.height > self.height {
                return;
            }
            for (row, byte) in glyph.bitmap.iter().enumerate() {
                let offset = (y + row) * self.pitch_pixels + x;
                for col in 0..8 {
                    if byte & (1 << (7 - col)) != 0 {
                        self.backbuffer[offset + col] = color;
                    }
                }
            }
        }
    }
}
