use crate::{
    clamp_i32, Bitmap, CompositorBackend, CompositorExport, DrawCommand, Rect, Rgba, Scene,
    CLEAR_COLOR, CURSOR_MASK, CURSOR_SIZE, FONT_HEIGHT, MAX_BACKBUFFER_PIXELS, SAFE_FB_HEIGHT,
    SAFE_FB_WIDTH,
};
use core::cmp::{max, min};
use unifont::get_glyph;

/// Framebuffer-based compositor backend for bare-metal builds.
///
/// This backend keeps a shadow backbuffer in regular memory and copies it to the
/// real framebuffer after rasterizing each scene.
pub struct FramebufferBackend {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) stride: usize,
    pub(crate) addr: *mut u32,
    pub(crate) backbuffer: &'static mut [u32],
}

impl FramebufferBackend {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        raw_width: usize,
        raw_height: usize,
        raw_pitch_bytes: usize,
        addr: *mut u32,
        backbuffer_storage: &'static mut [u32],
    ) -> Self {
        let fb_info = crate::sanitize_fb_info(crate::FramebufferInfo {
            width: raw_width,
            height: raw_height,
            pitch: raw_pitch_bytes,
            bpp: 32,
        });

        let mut stride = max(fb_info.pitch / 4, fb_info.width.max(1));
        let mut height = fb_info.height.max(1);

        if stride.saturating_mul(height) > MAX_BACKBUFFER_PIXELS {
            stride = SAFE_FB_WIDTH;
            height = SAFE_FB_HEIGHT;
        } else if stride.saturating_mul(height) > 2_000_000 {
            stride = max(SAFE_FB_WIDTH, fb_info.width);
            height = max(SAFE_FB_HEIGHT, fb_info.height);
        }

        let width = min(fb_info.width.max(1), stride);

        let needed = stride.saturating_mul(height).min(backbuffer_storage.len());
        let clamped_height = if stride == 0 { 0 } else { needed / stride };
        let backbuffer = &mut backbuffer_storage[..needed];

        Self {
            width,
            height: clamped_height,
            stride,
            addr,
            backbuffer,
        }
    }

    fn clear(&mut self, color: Rgba) {
        self.backbuffer.fill(color.to_u32());
    }

    fn present(&mut self) {
        if self.addr.is_null() || self.stride == 0 || self.height == 0 {
            return;
        }
        let fb = unsafe { core::slice::from_raw_parts_mut(self.addr, self.stride * self.height) };
        fb.copy_from_slice(self.backbuffer);
    }
}

impl CompositorBackend for FramebufferBackend {
    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn render(&mut self, scene: &Scene) {
        self.clear(CLEAR_COLOR);
        for cmd in scene.commands() {
            match cmd {
                DrawCommand::Clear { color } => {
                    self.clear(*color);
                }
                DrawCommand::FillRect { rect, color } => {
                    raster_fill_rect(self, rect, *color);
                }
                DrawCommand::BlitImage {
                    rect,
                    image,
                    repeat,
                } => {
                    raster_blit_image(self, rect, image, *repeat);
                }
                DrawCommand::DrawText {
                    origin,
                    text,
                    color,
                    max_width,
                } => {
                    raster_draw_text(self, *origin, text, *color, *max_width);
                }
                DrawCommand::DrawTextBlock { rect, text, color } => {
                    raster_draw_text_block(self, rect, text, *color);
                }
                DrawCommand::DrawCursor {
                    origin,
                    primary,
                    shadow,
                    pressed,
                } => {
                    raster_draw_cursor(self, *origin, *primary, *shadow, *pressed);
                }
            }
        }
        self.present();
    }

    fn export(&self) -> Option<CompositorExport<'_>> {
        Some(CompositorExport::Framebuffer {
            addr: self.addr as *const u32,
            width: self.width,
            height: self.height,
            stride_bytes: self.stride * 4,
        })
    }
}

fn raster_fill_rect(backend: &mut FramebufferBackend, rect: &Rect, color: Rgba) {
    if rect.width == 0 || rect.height == 0 || backend.stride == 0 {
        return;
    }
    let x0 = min(rect.x.max(0) as usize, backend.width);
    let y0 = min(rect.y.max(0) as usize, backend.height);
    let x1 = min(x0 + rect.width as usize, backend.width);
    let y1 = min(y0 + rect.height as usize, backend.height);
    let color_u32 = color.to_u32();
    for yy in y0..y1 {
        let row = yy * backend.stride;
        for xx in x0..x1 {
            backend.backbuffer[row + xx] = color_u32;
        }
    }
}

fn raster_blit_image(backend: &mut FramebufferBackend, rect: &Rect, bmp: &Bitmap, repeat: bool) {
    if rect.width == 0 || rect.height == 0 || backend.stride == 0 {
        return;
    }
    let x0 = min(rect.x.max(0) as usize, backend.width);
    let y0 = min(rect.y.max(0) as usize, backend.height);
    let x1 = min(x0 + rect.width as usize, backend.width);
    let y1 = min(y0 + rect.height as usize, backend.height);
    for yy in y0..y1 {
        let row = yy * backend.stride;
        for xx in x0..x1 {
            let sample_x = xx - x0;
            let sample_y = yy - y0;
            let color = if repeat {
                bmp.sample(sample_x, sample_y)
            } else if let Some(c) = bmp.pixel(sample_x, sample_y) {
                c
            } else {
                continue;
            };
            backend.backbuffer[row + xx] = color;
        }
    }
}

fn raster_draw_text(
    backend: &mut FramebufferBackend,
    origin: (i32, i32),
    text: &str,
    color: Rgba,
    max_width: Option<u32>,
) {
    if backend.stride == 0 {
        return;
    }
    let mut cursor_x = origin.0.max(0) as usize;
    let mut cursor_y = origin.1.max(0) as usize;
    let limit_x = max_width.map(|w| cursor_x + w as usize);
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = origin.0.max(0) as usize;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT);
            continue;
        }
        let Some(glyph) = get_glyph(ch) else { continue };
        let gw = glyph.get_width();
        if let Some(limit) = limit_x {
            if cursor_x + gw > limit {
                break;
            }
        }
        raster_draw_glyph(backend, cursor_x, cursor_y, glyph, color.to_u32());
        cursor_x += gw;
    }
}

fn raster_draw_text_block(backend: &mut FramebufferBackend, rect: &Rect, text: &str, color: Rgba) {
    if backend.stride == 0 {
        return;
    }
    let mut cursor_x = rect.x.max(0) as usize;
    let mut cursor_y = rect.y.max(0) as usize;
    let max_x = rect.x.max(0) as usize + rect.width as usize;
    let max_y = rect.y.max(0) as usize + rect.height as usize;
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = rect.x.max(0) as usize;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT);
            if cursor_y + FONT_HEIGHT >= max_y {
                break;
            }
            continue;
        }
        let Some(glyph) = get_glyph(ch) else { continue };
        let gw = glyph.get_width();
        if cursor_x + gw >= max_x {
            cursor_x = rect.x.max(0) as usize;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT);
            if cursor_y + FONT_HEIGHT >= max_y {
                break;
            }
        }
        raster_draw_glyph(backend, cursor_x, cursor_y, glyph, color.to_u32());
        cursor_x += gw;
    }
}

fn raster_draw_glyph(
    backend: &mut FramebufferBackend,
    x: usize,
    y: usize,
    glyph: &unifont::Glyph,
    color: u32,
) {
    if x >= backend.width || y >= backend.height || backend.stride == 0 {
        return;
    }

    let width = glyph.get_width();
    for row in 0..FONT_HEIGHT {
        let dst_y = y + row;
        if dst_y >= backend.height {
            break;
        }
        for col in 0..width {
            let dst_x = x + col;
            if dst_x >= backend.width {
                break;
            }
            if glyph.get_pixel(col, row) {
                let idx = dst_y * backend.stride + dst_x;
                backend.backbuffer[idx] = color;
            }
        }
    }
}

fn raster_draw_cursor(
    backend: &mut FramebufferBackend,
    origin: (i32, i32),
    primary: Rgba,
    shadow: Rgba,
    pressed: bool,
) {
    if backend.stride == 0 {
        return;
    }
    let base_x = clamp_i32(origin.0, 0, backend.width.saturating_sub(1) as i32) as usize;
    let base_y = clamp_i32(origin.1, 0, backend.height.saturating_sub(1) as i32) as usize;

    let primary_color = primary.to_u32();
    let shadow_color = shadow.to_u32();
    let draw_shadow = !pressed;

    for (row, mask) in CURSOR_MASK.iter().enumerate() {
        let y = base_y + row;
        if y >= backend.height {
            break;
        }
        for col in 0..CURSOR_SIZE {
            let bit = 15 - col;
            if (mask & (1 << bit)) == 0 {
                continue;
            }

            let x = base_x + col;
            if x >= backend.width {
                break;
            }

            if draw_shadow && x + 1 < backend.width && y + 1 < backend.height {
                let shadow_idx = (y + 1) * backend.stride + (x + 1);
                backend.backbuffer[shadow_idx] = shadow_color;
            }

            let idx = y * backend.stride + x;
            backend.backbuffer[idx] = primary_color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{boxed::Box, vec};

    #[test]
    fn bitmap_pixel_respects_bounds() {
        let bmp = Bitmap::new(2, 2, vec![1, 2, 3, 4]);

        assert_eq!(bmp.pixel(0, 0), Some(1));
        assert_eq!(bmp.pixel(1, 1), Some(4));
        assert_eq!(bmp.pixel(2, 0), None);
        assert_eq!(bmp.pixel(0, 2), None);
    }

    #[test]
    fn raster_blit_image_skips_out_of_bounds_when_not_repeating() {
        let storage = vec![0u32; 9];
        let backbuffer: &'static mut [u32] = Box::leak(storage.into_boxed_slice());
        let mut backend = FramebufferBackend::new(
            3,
            3,
            3 * core::mem::size_of::<u32>(),
            core::ptr::null_mut(),
            backbuffer,
        );
        let bmp = Bitmap::new(2, 2, vec![1, 2, 3, 4]);

        raster_blit_image(&mut backend, &Rect::new(0, 0, 3, 3), &bmp, false);

        assert_eq!(backend.backbuffer, &[1, 2, 0, 3, 4, 0, 0, 0, 0]);
    }
}
