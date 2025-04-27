use embedded_graphics::prelude::DrawTarget;
use serde::Serialize;

use crate::framebuffer::Framebuffer;
use crate::gui_output::GuiOutputBuffer;

#[derive(Clone, Serialize)]
pub struct Screen {
    pub buffer: GuiOutputBuffer,
}

impl Screen {
    pub fn new(framebuffer: &Framebuffer) -> Self {
        Self {
            buffer: GuiOutputBuffer::from_framebuffer(framebuffer),
        }
    }

    /// Blit the current screen buffer to the real framebuffer
    pub fn blit_to(&self, framebuffer: &mut Framebuffer) {
        self.buffer.blit_to(framebuffer);
    }

    /// Expose buffer for drawing
    pub fn buffer_mut(&mut self) -> &mut GuiOutputBuffer {
        &mut self.buffer
    }

    /// Clear the screen
    pub fn clear(&mut self, color: embedded_graphics::pixelcolor::Rgb565) {
        self.buffer
            .draw_iter(core::iter::empty::<embedded_graphics::Pixel<_>>())
            .ok();
        let encoded = super::gui_output::encode_color_rgb565(color);
        for px in self.buffer.pixels.iter_mut() {
            *px = encoded;
        }
    }
}
