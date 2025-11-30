use crate::{
    clamp_i32, Bitmap, FrameInfo, FramebufferDevice, FramebufferGeometry, Rect, RendererBackend,
    Rgba, Scene, SceneItem, CLEAR_COLOR, FONT_HEIGHT,
};
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};
use unifont::get_glyph;

pub struct BitmapRenderer {
    width: usize,
    height: usize,
    storage: Vec<u32>,
}

pub struct BitmapFramebufferDevice {
    width: usize,
    height: usize,
    pitch: usize,
    addr: *mut u32,
}

unsafe impl Send for BitmapFramebufferDevice {}
unsafe impl Sync for BitmapFramebufferDevice {}

impl BitmapRenderer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            storage: vec![0; width * height],
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.storage.resize(width * height, 0);
    }

    fn clear(&mut self, color: Rgba) {
        self.storage.fill(color.to_u32());
    }
}

impl RendererBackend for BitmapRenderer {
    type Output<'b>
        = &'b [u32]
    where
        Self: 'b;
    fn render<'b>(&'b mut self, scene: &Scene) -> Self::Output<'b> {
        self.clear(CLEAR_COLOR);
        let mut clip_stack: Vec<Option<Rect>> = Vec::new();
        clip_stack.push(Some(Rect::new(0, 0, self.width as u32, self.height as u32)));

        for item in scene.items() {
            match item {
                SceneItem::Clear { color } => {
                    self.clear(*color);
                }
                SceneItem::FillRect { rect, color } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_fill_rect(self, rect, *color, clip);
                }
                SceneItem::BlitImage {
                    rect,
                    image,
                    repeat,
                    offset,
                } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_blit_image(self, rect, image, *repeat, *offset, clip);
                }
                SceneItem::DrawText {
                    origin,
                    text,
                    color,
                    max_width,
                } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_draw_text(self, *origin, text, *color, *max_width, clip);
                }
                SceneItem::DrawTextBlock {
                    rect,
                    text,
                    color,
                    scroll_offset,
                } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_draw_text_block(self, rect, text, *color, *scroll_offset, clip);
                }
                SceneItem::DrawCursor {
                    origin,
                    sprite,
                    hotspot,
                } => {
                    raster_draw_cursor(self, *origin, sprite, *hotspot);
                }
                SceneItem::ClipPush { rect } => {
                    let parent_clip = clip_stack.last().copied().flatten();
                    let new_clip = parent_clip.and_then(|base| intersect_rect(base, *rect));
                    clip_stack.push(new_clip);
                }
                SceneItem::ClipPop => {
                    if clip_stack.len() > 1 {
                        clip_stack.pop();
                    }
                }
            }
        }
        &self.storage
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

fn raster_fill_rect(backend: &mut BitmapRenderer, rect: &Rect, color: Rgba, clip: Option<Rect>) {
    if rect.width == 0 || rect.height == 0 || backend.width == 0 {
        return;
    }
    let clipped = apply_clip(*rect, clip);
    let Some(rect) = clipped else {
        return;
    };
    let x0 = min(rect.x.max(0) as usize, backend.width);
    let y0 = min(rect.y.max(0) as usize, backend.height);
    let x1 = min(x0 + rect.width as usize, backend.width);
    let y1 = min(y0 + rect.height as usize, backend.height);
    let color_u32 = color.to_u32();

    if color.a == 0xFF {
        for yy in y0..y1 {
            let row = yy * backend.width;
            for xx in x0..x1 {
                backend.storage[row + xx] = color_u32;
            }
        }
    } else {
        for yy in y0..y1 {
            let row = yy * backend.width;
            for xx in x0..x1 {
                let bg = backend.storage[row + xx];
                backend.storage[row + xx] = userland::graphics::blend(color_u32, bg);
            }
        }
    }
}

fn raster_blit_image(
    backend: &mut BitmapRenderer,
    rect: &Rect,
    bmp: &Bitmap,
    repeat: bool,
    offset: (i32, i32),
    clip: Option<Rect>,
) {
    if rect.width == 0 || rect.height == 0 || backend.width == 0 {
        return;
    }
    let clipped = apply_clip(*rect, clip);
    let Some(target_rect) = clipped else {
        return;
    };
    let x0 = min(target_rect.x.max(0) as usize, backend.width);
    let y0 = min(target_rect.y.max(0) as usize, backend.height);
    let x1 = min(x0 + target_rect.width as usize, backend.width);
    let y1 = min(y0 + target_rect.height as usize, backend.height);
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
    clip: Option<Rect>,
) {
    if backend.width == 0 {
        return;
    }
    let mut cursor_x = origin.0;
    let mut cursor_y = origin.1;
    let limit_x = max_width.map(|w| origin.0 + w as i32);
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = origin.0;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT as i32);
            continue;
        }
        let Some(glyph) = get_glyph(ch) else { continue };
        let gw = glyph.get_width() as i32;
        if let Some(limit) = limit_x {
            if cursor_x + gw > limit {
                break;
            }
        }
        raster_draw_glyph(backend, cursor_x, cursor_y, glyph, color.to_u32(), clip);
        cursor_x += gw;
    }
}

fn raster_draw_text_block(
    backend: &mut BitmapRenderer,
    rect: &Rect,
    text: &str,
    color: Rgba,
    scroll_offset: i32,
    clip: Option<Rect>,
) {
    if backend.width == 0 {
        return;
    }
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let clip_bounds = match clip {
        Some(active_clip) => intersect_rect(*rect, active_clip),
        None => Some(*rect),
    };
    let clip_bounds = match clip_bounds {
        Some(bounds) => bounds,
        None => return,
    };
    let content_width = rect.width as i32;
    let view_top = rect.y;
    let view_bottom = rect.y + rect.height as i32;
    let mut cursor_x: i32 = 0;
    let mut cursor_y: i32 = 0;
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = 0;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT as i32);
            if rect.y + cursor_y - scroll_offset >= view_bottom {
                break;
            }
            continue;
        }
        let Some(glyph) = get_glyph(ch) else { continue };
        let gw = glyph.get_width() as i32;
        if cursor_x + gw > content_width {
            cursor_x = 0;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT as i32);
        }
        let draw_y = rect.y + cursor_y - scroll_offset;
        if draw_y >= view_bottom || draw_y >= clip_bounds.y + clip_bounds.height as i32 {
            break;
        }
        if draw_y + FONT_HEIGHT as i32 > view_top && draw_y + FONT_HEIGHT as i32 > clip_bounds.y {
            let draw_x = rect.x + cursor_x;
            raster_draw_glyph(
                backend,
                draw_x,
                draw_y,
                glyph,
                color.to_u32(),
                Some(clip_bounds),
            );
        }
        cursor_x += gw;
    }
}

fn raster_draw_glyph(
    backend: &mut BitmapRenderer,
    x: i32,
    y: i32,
    glyph: &unifont::Glyph,
    color: u32,
    clip: Option<Rect>,
) {
    if backend.width == 0 || backend.height == 0 {
        return;
    }

    let glyph_width = glyph.get_width() as i32;
    for row in 0..FONT_HEIGHT as i32 {
        let dst_y = y + row;
        if dst_y < 0 {
            continue;
        }
        if dst_y >= backend.height as i32 {
            break;
        }
        if let Some(clip_rect) = clip {
            if dst_y < clip_rect.y {
                continue;
            }
            if dst_y >= clip_rect.y + clip_rect.height as i32 {
                break;
            }
        }
        for col in 0..glyph_width {
            let dst_x = x + col;
            if dst_x < 0 || dst_x >= backend.width as i32 {
                continue;
            }
            if let Some(clip_rect) = clip {
                if dst_x < clip_rect.x || dst_x >= clip_rect.x + clip_rect.width as i32 {
                    continue;
                }
            }
            if glyph.get_pixel(col as usize, row as usize) {
                let idx = dst_y as usize * backend.width + dst_x as usize;
                backend.storage[idx] = color;
            }
        }
    }
}

fn raster_draw_cursor(
    backend: &mut BitmapRenderer,
    origin: (i32, i32),
    sprite: &Bitmap,
    hotspot: (i32, i32),
) {
    if backend.width == 0 || sprite.width == 0 || sprite.height == 0 {
        return;
    }

    let top_left_x = origin.0 - hotspot.0;
    let top_left_y = origin.1 - hotspot.1;

    let start_x = clamp_i32(top_left_x, 0, backend.width as i32);
    let start_y = clamp_i32(top_left_y, 0, backend.height as i32);
    let end_x = clamp_i32(top_left_x + sprite.width as i32, 0, backend.width as i32);
    let end_y = clamp_i32(top_left_y + sprite.height as i32, 0, backend.height as i32);

    for y in start_y..end_y {
        for x in start_x..end_x {
            let sx = (x - top_left_x) as usize;
            let sy = (y - top_left_y) as usize;
            let Some(px) = sprite.pixel(sx, sy) else {
                continue;
            };
            let alpha = (px >> 24) & 0xFF;
            if alpha == 0 {
                continue;
            }
            let idx = (y as usize) * backend.width + (x as usize);
            let dst = backend.storage[idx];
            let out = if alpha == 0xFF {
                px
            } else {
                let color = (alpha << 24) | (px & 0x00FFFFFF);
                userland::graphics::blend(color, dst)
            };
            backend.storage[idx] = out;
        }
    }
}

fn apply_clip(rect: Rect, clip: Option<Rect>) -> Option<Rect> {
    clip.and_then(|clip_rect| intersect_rect(rect, clip_rect))
}

fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = max(a.x, b.x);
    let y0 = max(a.y, b.y);
    let x1 = min(a.x + a.width as i32, b.x + b.width as i32);
    let y1 = min(a.y + a.height as i32, b.y + b.height as i32);

    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
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
        let mut backend = BitmapRenderer::new(3, 3);
        let bmp = Bitmap::new(2, 2, vec![1, 2, 3, 4]);

        raster_blit_image(&mut backend, &Rect::new(0, 0, 3, 3), &bmp, false, None);

        assert_eq!(backend.storage, &[1, 2, 0, 3, 4, 0, 0, 0, 0]);
    }
}
