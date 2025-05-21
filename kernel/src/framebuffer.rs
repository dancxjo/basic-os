use core::convert::Infallible;
use embedded_graphics::{
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
};
use limine::request::FramebufferRequest;

const MAX_WIDTH: usize = 3840;
const MAX_HEIGHT: usize = 2160;

#[unsafe(link_section = ".bss.uninit")]
#[unsafe(no_mangle)]
pub static mut BACKBUFFER: [u32; MAX_WIDTH * MAX_HEIGHT] = [0; MAX_WIDTH * MAX_HEIGHT];

#[derive(Debug)]
pub struct Framebuffer {
    fb: &'static mut [u32],
    backbuffer: &'static mut [u32],
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub pitch_pixels: usize,
    pub bpp: u16,
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
                self.backbuffer[index] = self.encode_color_rgb565(color);
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
    pub fn new() -> Option<Self> {
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

        Some(Self {
            fb: fb_slice,
            backbuffer,
            width,
            height,
            pitch,
            pitch_pixels,
            bpp: fb_info.bpp(),
        })
    }

    pub fn encode_color_rgb565(&self, color: Rgb565) -> u32 {
        match self.bpp {
            16 => {
                let raw: u16 =
                    ((color.r() as u16) << 11) | ((color.g() as u16) << 5) | (color.b() as u16);
                raw as u32
            }
            24 | 32 => {
                // Scale up the 5/6/5-bit color to 8-bit
                let r = (color.r() as u32) << 3;
                let g = (color.g() as u32) << 2;
                let b = (color.b() as u32) << 3;
                (r << 16) | (g << 8) | b
            }
            _ => 0,
        }
    }

    pub fn flush(&mut self) {
        self.fb.copy_from_slice(&self.backbuffer[..self.fb.len()]);
    }

    /// Restore the framebuffer to the values in the backbuffer for a specific region
    pub fn erase_region(&mut self, x: usize, y: usize, width: usize, height: usize) {
        let start = y * self.pitch_pixels + x;
        let end = start + (height * self.pitch_pixels) + width;
        self.fb[start..end].copy_from_slice(&self.backbuffer[start..end]);
    }

    pub fn dangerous_direct_access_mut(&mut self) -> &mut [u32] {
        self.fb
    }

    pub fn clear(&mut self, color: Rgb565) {
        let encoded = self.encode_color_rgb565(color);
        for px in self.backbuffer.iter_mut() {
            *px = encoded;
        }
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

    pub fn backbuffer_mut(&mut self) -> &mut [u32] {
        self.backbuffer
    }
}
