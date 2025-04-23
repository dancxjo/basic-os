use alloc::{boxed::Box, vec};
use core::convert::Infallible;
use embedded_graphics::{Pixel, pixelcolor::Rgb565, prelude::*};

pub struct GuiOutputBuffer {
    pub pixels: Box<[u32]>,
    pub width: usize,
    pub height: usize,
    pub pitch_pixels: usize,
}

impl GuiOutputBuffer {
    /// Create a new owned output buffer
    pub fn new(width: usize, height: usize, pitch_pixels: usize) -> Self {
        let len = pitch_pixels * height;
        let pixels = vec![0u32; len].into_boxed_slice();

        GuiOutputBuffer {
            pixels,
            width,
            height,
            pitch_pixels,
        }
    }

    /// Create a new buffer with the same size as the framebuffer
    pub fn from_framebuffer(framebuffer: &crate::framebuffer::Framebuffer) -> Self {
        Self::new(
            framebuffer.width(),
            framebuffer.height(),
            framebuffer.pitch_pixels(),
        )
    }

    /// Copy this buffer's pixels into another target (like the framebuffer)
    pub fn blit_to(&self, framebuffer: &mut crate::framebuffer::Framebuffer) {
        let fb_len = framebuffer.fb_len();
        let fb_mut = framebuffer.fb_mut();
        fb_mut.copy_from_slice(&self.pixels[..fb_len]);
    }
}

impl DrawTarget for GuiOutputBuffer {
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
                self.pixels[index] = encode_color_rgb565(color);
            }
        }
        Ok(())
    }
}

impl OriginDimensions for GuiOutputBuffer {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

fn encode_color_rgb565(color: Rgb565) -> u32 {
    let r = (color.r() as u32) << 3;
    let g = (color.g() as u32) << 2;
    let b = (color.b() as u32) << 3;
    (r << 16) | (g << 8) | b
}
