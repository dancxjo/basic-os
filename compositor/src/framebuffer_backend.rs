use crate::{
    clamp_i32, Bitmap, FrameInfo, FramebufferDevice, FramebufferGeometry, Rect, RendererBackend,
    Rgba, Scene, SceneItem, CLEAR_COLOR, CURSOR_MASK, CURSOR_SIZE, FONT_HEIGHT,
};
use core::cmp::min;
use unifont::get_glyph;

pub struct BitmapRenderer<'a> {
    width: usize,
    height: usize,
    storage: &'a mut [u32],
}

pub struct BitmapFramebufferDevice {
    width: usize,
    height: usize,
    pitch: usize,
    addr: *mut u32,
}

unsafe impl Send for BitmapFramebufferDevice {}
unsafe impl Sync for BitmapFramebufferDevice {}

impl<'a> BitmapRenderer<'a> {
    pub fn new(width: usize, height: usize, storage: &'a mut [u32]) -> Self {
        Self {
            width,
            height,
            storage,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if width.saturating_mul(height) <= self.storage.len() {
            self.width = width;
            self.height = height;
        }
    }

    fn clear(&mut self, color: Rgba) {
        let needed = self.width * self.height;
        if needed <= self.storage.len() {
            self.storage[..needed].fill(color.to_u32());
        }
    }
}

impl<'a> RendererBackend for BitmapRenderer<'a> {
    type Output<'b>
        = &'b [u32]
    where
        Self: 'b;
    fn render<'b>(&'b mut self, scene: &Scene) -> Self::Output<'b> {
        self.clear(CLEAR_COLOR);
        for item in scene.items() {
            match item {
                SceneItem::Clear { color } => {
                    self.clear(*color);
                }
                SceneItem::FillRect { rect, color } => {
                    raster_fill_rect(self, rect, *color);
                }
                SceneItem::BlitImage {
                    rect,
                    image,
                    repeat,
                    offset,
                } => {
                    raster_blit_image(self, rect, image, *repeat, *offset);
                }
                SceneItem::DrawText {
                    origin,
                    text,
                    color,
                    max_width,
                } => {
                    raster_draw_text(self, *origin, text, *color, *max_width);
                }
                SceneItem::DrawTextBlock { rect, text, color } => {
                    raster_draw_text_block(self, rect, text, *color);
                }
                SceneItem::DrawCursor {
                    origin,
                    primary,
                    shadow,
                    pressed,
                } => {
                    raster_draw_cursor(self, *origin, *primary, *shadow, *pressed);
                }
            }
        }
        let needed = self.width * self.height;
        if needed <= self.storage.len() {
            &self.storage[..needed]
        } else {
            &self.storage[..]
        }
    }
}

impl BitmapFramebufferDevice {
    pub fn new(
        raw_width: usize,
        raw_height: usize,
        raw_pitch_bytes: usize,
        addr: *mut u32,
    ) -> Self {
        let fb_info = crate::sanitize_fb_info(crate::FramebufferGeometry {
            width: raw_width as u32,
            height: raw_height as u32,
            pitch: raw_pitch_bytes as u32,
            bpp: 32,
        });

        Self {
            width: fb_info.width as usize,
            height: fb_info.height as usize,
            pitch: fb_info.pitch as usize,
            addr,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize, pitch: usize, addr: *mut u32) {
        self.width = width;
        self.height = height;
        self.pitch = pitch;
        self.addr = addr;
    }
}

impl<'a> FramebufferDevice<&'a [u32]> for BitmapFramebufferDevice {
    fn geometry(&self) -> FramebufferGeometry {
        FramebufferGeometry {
            width: self.width as u32,
            height: self.height as u32,
            pitch: self.pitch as u32,
            bpp: 32,
        }
    }

    fn present(&mut self, frame: &'a [u32]) {
        if self.addr.is_null() || self.pitch == 0 || self.height == 0 {
            return;
        }

        let stride_u32 = self.pitch / 4;
        let copy_width = min(self.width, stride_u32);
        let copy_height = min(self.height, frame.len() / self.width);

        for y in 0..copy_height {
            let src_start = y * self.width;
            let src_end = src_start + copy_width;
            let dst_start = y * stride_u32;

            if src_end <= frame.len() {
                let src_row = &frame[src_start..src_end];
                unsafe {
                    let dst_ptr = self.addr.add(dst_start);
                    let dst_row = core::slice::from_raw_parts_mut(dst_ptr, copy_width);
                    dst_row.copy_from_slice(src_row);
                }
            }
        }
    }

    fn frame_info(&self) -> Option<FrameInfo> {
        Some(FrameInfo {
            addr: self.addr as u64,
            width: self.width as u64,
            height: self.height as u64,
            pitch: self.pitch as u64,
            bpp: 32,
        })
    }
}

fn raster_fill_rect(backend: &mut BitmapRenderer, rect: &Rect, color: Rgba) {
    if rect.width == 0 || rect.height == 0 || backend.width == 0 {
        return;
    }
    let x0 = min(rect.x.max(0) as usize, backend.width);
    let y0 = min(rect.y.max(0) as usize, backend.height);
    let x1 = min(x0 + rect.width as usize, backend.width);
    let y1 = min(y0 + rect.height as usize, backend.height);
    let color_u32 = color.to_u32();
    for yy in y0..y1 {
        let row = yy * backend.width;
        for xx in x0..x1 {
            backend.storage[row + xx] = color_u32;
        }
    }
}

fn raster_blit_image(
    backend: &mut BitmapRenderer,
    rect: &Rect,
    bmp: &Bitmap,
    repeat: bool,
    offset: (i32, i32),
) {
    if rect.width == 0 || rect.height == 0 || backend.width == 0 {
        return;
    }
    let x0 = min(rect.x.max(0) as usize, backend.width);
    let y0 = min(rect.y.max(0) as usize, backend.height);
    let x1 = min(x0 + rect.width as usize, backend.width);
    let y1 = min(y0 + rect.height as usize, backend.height);
    for yy in y0..y1 {
        let row = yy * backend.width;
        for xx in x0..x1 {
            let sample_x = (xx as i32 - rect.x + offset.0) as usize;
            let sample_y = (yy as i32 - rect.y + offset.1) as usize;
            let color = if repeat {
                bmp.sample(sample_x, sample_y)
            } else if let Some(c) = bmp.pixel(sample_x, sample_y) {
                c
            } else {
                continue;
            };
            backend.storage[row + xx] = color;
        }
    }
}

fn raster_draw_text(
    backend: &mut BitmapRenderer,
    origin: (i32, i32),
    text: &str,
    color: Rgba,
    max_width: Option<u32>,
) {
    if backend.width == 0 {
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

fn raster_draw_text_block(backend: &mut BitmapRenderer, rect: &Rect, text: &str, color: Rgba) {
    if backend.width == 0 {
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
    backend: &mut BitmapRenderer,
    x: usize,
    y: usize,
    glyph: &unifont::Glyph,
    color: u32,
) {
    if x >= backend.width || y >= backend.height || backend.width == 0 {
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
                let idx = dst_y * backend.width + dst_x;
                backend.storage[idx] = color;
            }
        }
    }
}

fn raster_draw_cursor(
    backend: &mut BitmapRenderer,
    origin: (i32, i32),
    primary: Rgba,
    shadow: Rgba,
    pressed: bool,
) {
    if backend.width == 0 {
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
                let shadow_idx = (y + 1) * backend.width + (x + 1);
                backend.storage[shadow_idx] = shadow_color;
            }

            let idx = y * backend.width + x;
            backend.storage[idx] = primary_color;
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
        let mut backend = BitmapRenderer::new(3, 3, backbuffer);
        let bmp = Bitmap::new(2, 2, vec![1, 2, 3, 4]);

        raster_blit_image(&mut backend, &Rect::new(0, 0, 3, 3), &bmp, false);

        assert_eq!(backend.storage, &[1, 2, 0, 3, 4, 0, 0, 0, 0]);
    }
}
