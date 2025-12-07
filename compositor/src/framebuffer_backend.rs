use crate::fonts::font_manager;
use crate::{
    clamp_i32, Bitmap, FrameInfo, FramebufferDevice, FramebufferGeometry, Rect, RendererBackend,
    Rgba, Scene, SceneItem, CLEAR_COLOR, FONT_HEIGHT, SCROLLBAR_THUMB_COLOR,
    SCROLLBAR_THUMB_HILIGHT, SCROLLBAR_THUMB_SHADOW, SCROLLBAR_TRACK_COLOR,
};
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};

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

#[derive(Clone, Copy)]
enum ClearMode {
    Full,
    Clip(Rect),
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

    fn render_scene<'a>(
        &'a mut self,
        scene: &Scene,
        root_clip: Rect,
        clear_mode: ClearMode,
    ) -> &'a [u32] {
        let mut clip_stack: Vec<Option<Rect>> = Vec::new();
        clip_stack.push(Some(root_clip));

        for item in &scene.items {
            match item {
                SceneItem::Clear { color } => match clear_mode {
                    ClearMode::Full => {
                        self.clear(*color);
                    }
                    ClearMode::Clip(rect) => {
                        raster_fill_rect(self, &rect, *color, Some(rect));
                    }
                },
                SceneItem::FillRect { rect, color } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_fill_rect(self, rect, *color, clip);
                }
                SceneItem::HatchRect {
                    rect,
                    color,
                    spacing,
                } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_hatch_rect(self, rect, *color, *spacing, clip);
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
                    is_mono,
                } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_draw_text(self, *origin, text, *color, *max_width, clip, *is_mono);
                }
                SceneItem::DrawTextBlock {
                    rect,
                    text,
                    color,
                    scroll_offset,
                    is_mono,
                } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_draw_text_block(
                        self,
                        rect,
                        text,
                        *color,
                        *scroll_offset,
                        clip,
                        *is_mono,
                    );
                }
                SceneItem::DrawCursor {
                    origin,
                    sprite,
                    hotspot,
                } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_draw_cursor(self, *origin, sprite, *hotspot, clip);
                }
                SceneItem::DrawLine { start, end, color } => {
                    let clip = clip_stack.last().copied().flatten();
                    raster_draw_line(self, *start, *end, *color, clip);
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

impl RendererBackend for BitmapRenderer {
    type Output<'b>
        = &'b [u32]
    where
        Self: 'b;
    fn render<'a>(&'a mut self, scene: &Scene) -> Self::Output<'a> {
        self.clear(CLEAR_COLOR);
        self.render_scene(
            scene,
            Rect::new(0, 0, self.width as u32, self.height as u32),
            ClearMode::Full,
        )
    }

    fn render_partial<'a>(&'a mut self, scene: &Scene, dirty_rect: Rect) -> Self::Output<'a> {
        self.render_scene(scene, dirty_rect, ClearMode::Clip(dirty_rect))
    }
}

fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let x = max(a.x, b.x);
    let y = max(a.y, b.y);
    let x2 = min(a.x + a.width as i32, b.x + b.width as i32);
    let y2 = min(a.y + a.height as i32, b.y + b.height as i32);

    if x < x2 && y < y2 {
        Some(Rect::new(x, y, (x2 - x) as u32, (y2 - y) as u32))
    } else {
        None
    }
}

fn apply_clip(rect: Rect, clip: Option<Rect>) -> Option<Rect> {
    if let Some(c) = clip {
        intersect_rect(rect, c)
    } else {
        Some(rect)
    }
}

fn raster_hatch_rect(
    backend: &mut BitmapRenderer,
    rect: &Rect,
    color: Rgba,
    spacing: i32,
    clip: Option<Rect>,
) {
    if rect.width == 0 || rect.height == 0 || backend.width == 0 || spacing <= 0 {
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

    // Simple diagonal hatch: (x + y) % spacing == 0
    for yy in y0..y1 {
        let row = yy * backend.width;
        for xx in x0..x1 {
            if (xx as i32 + yy as i32) % spacing == 0 {
                backend.storage[row + xx] = color_u32;
            }
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

    fn present_partial(&mut self, frame: &'a [u32], dirty_rect: Rect) {
        if self.addr.is_null() || self.pitch == 0 || self.height == 0 {
            return;
        }

        let stride_u32 = self.pitch / 4;

        // Clip dirty rect to framebuffer bounds
        let fb_rect = Rect::new(0, 0, self.width as u32, self.height as u32);
        let Some(rect) = intersect_rect(dirty_rect, fb_rect) else {
            return;
        };

        let x = rect.x as usize;
        let y = rect.y as usize;
        let w = rect.width as usize;
        let h = rect.height as usize;

        for row in 0..h {
            let curr_y = y + row;
            let src_start = curr_y * self.width + x;
            let src_end = src_start + w;
            let dst_start = curr_y * stride_u32 + x;

            if src_end <= frame.len() {
                let src_row = &frame[src_start..src_end];
                unsafe {
                    let dst_ptr = self.addr.add(dst_start);
                    let dst_row = core::slice::from_raw_parts_mut(dst_ptr, w);
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
            let alpha = (color >> 24) & 0xFF;
            if alpha == 0 {
                continue;
            } else if alpha == 0xFF {
                backend.storage[row + xx] = color;
            } else {
                let bg = backend.storage[row + xx];
                backend.storage[row + xx] = userland::graphics::blend(color, bg);
            }
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
    is_mono: bool,
) {
    if backend.width == 0 {
        return;
    }
    let fm = font_manager();
    let size = FONT_HEIGHT as f32;
    let metrics = if is_mono {
        fm.mono_line_metrics(size)
    } else {
        fm.line_metrics(size)
    };
    let ascent = metrics.ascent;
    let new_line_size = metrics.new_line_size;

    let mut cursor_x = origin.0 as f32;
    let mut baseline_y = origin.1 as f32 + ascent;
    let limit_x = max_width.map(|w| origin.0 as f32 + w as f32);

    if is_mono {
        // println!("raster_draw_text mono: text='{}' origin={:?} size={}", text, origin, size);
    }

    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = origin.0 as f32;
            baseline_y += new_line_size;
            continue;
        }
        let (metrics, bitmap) = if is_mono {
            fm.rasterize_mono(ch, size)
        } else {
            fm.rasterize(ch, size)
        };
        let gw = metrics.advance_width;

        if is_mono && bitmap.is_empty() && ch != ' ' {
            // println!("raster_draw_text mono: empty bitmap for '{}'", ch);
        }

        if let Some(limit) = limit_x {
            if cursor_x + gw > limit {
                break;
            }
        }

        let draw_x = (cursor_x + metrics.xmin as f32) as i32;
        let draw_y = (baseline_y - (metrics.ymin as f32 + metrics.height as f32)) as i32;

        raster_draw_glyph(
            backend,
            draw_x,
            draw_y,
            &bitmap,
            metrics.width as i32,
            metrics.height as i32,
            color.to_u32(),
            clip,
        );
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
    is_mono: bool,
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

    let fm = font_manager();
    let size = FONT_HEIGHT as f32;
    let line_metrics = if is_mono {
        fm.mono_line_metrics(size)
    } else {
        fm.line_metrics(size)
    };
    let ascent = line_metrics.ascent;
    let new_line_size = line_metrics.new_line_size;

    let mut cursor_x: f32 = 0.0;
    let mut current_y: f32 = 0.0;

    if is_mono {
        userland::println!(
            "raster_draw_text_block mono: text len={} rect={:?} color={:x}",
            text.len(),
            rect,
            color.to_u32()
        );
    }

    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = 0.0;
            current_y += new_line_size;
            if rect.y + current_y as i32 - scroll_offset >= view_bottom {
                break;
            }
            continue;
        }
        let (metrics, bitmap) = if is_mono {
            fm.rasterize_mono(ch, size)
        } else {
            fm.rasterize(ch, size)
        };

        if is_mono && text.len() < 100 {
            userland::println!(
                "  ch='{}' w={} h={} bmp_len={}",
                ch,
                metrics.width,
                metrics.height,
                bitmap.len()
            );
        }

        let gw = metrics.advance_width;

        if cursor_x + gw > content_width as f32 {
            cursor_x = 0.0;
            current_y += new_line_size;
        }

        let baseline_y = rect.y as f32 + current_y + ascent - scroll_offset as f32;
        let draw_y = (baseline_y - (metrics.ymin as f32 + metrics.height as f32)) as i32;

        if draw_y >= view_bottom {
            break;
        }

        // Only draw if visible
        if draw_y + metrics.height as i32 > view_top && draw_y < view_bottom {
            let draw_x = rect.x + (cursor_x + metrics.xmin as f32) as i32;
            raster_draw_glyph(
                backend,
                draw_x,
                draw_y,
                &bitmap,
                metrics.width as i32,
                metrics.height as i32,
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
    bitmap: &[u8],
    width: i32,
    height: i32,
    color: u32,
    clip: Option<Rect>,
) {
    if backend.width == 0 || backend.height == 0 || width == 0 || height == 0 {
        return;
    }

    for row in 0..height {
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
        for col in 0..width {
            let dst_x = x + col;
            if dst_x < 0 || dst_x >= backend.width as i32 {
                continue;
            }
            if let Some(clip_rect) = clip {
                if dst_x < clip_rect.x || dst_x >= clip_rect.x + clip_rect.width as i32 {
                    continue;
                }
            }

            let alpha = bitmap[(row * width + col) as usize];
            if alpha == 0 {
                continue;
            }

            let idx = dst_y as usize * backend.width + dst_x as usize;
            if alpha == 255 {
                backend.storage[idx] = color;
            } else {
                // Blend
                let bg = backend.storage[idx];
                // Should blend color using alpha to produce source, then blend source over bg?
                // Actually color has alpha too (usually 0xFF).
                // Let's assume color is the source color (e.g. black text).
                // The glyph alpha is the coverage.
                // Final alpha = color.a * glyph_alpha.
                // We are blending (color with coverage) over bg.
                let fg_a = ((color >> 24) & 0xFF) as u32;
                let final_a = (fg_a * alpha as u32) / 255;
                if final_a == 0 {
                    continue;
                }

                // Construct source pixel with modified alpha?
                // userland::graphics::blend expects src and dst.
                // We should modify 'color' to have 'final_a'.
                let src = (color & 0x00FFFFFF) | (final_a << 24);

                backend.storage[idx] = userland::graphics::blend(src, bg);
            }
        }
    }
}

fn raster_draw_cursor(
    backend: &mut BitmapRenderer,
    origin: (i32, i32),
    sprite: &Bitmap,
    hotspot: (i32, i32),
    clip: Option<Rect>,
) {
    if backend.width == 0 || sprite.width == 0 || sprite.height == 0 {
        return;
    }

    let top_left_x = origin.0 - hotspot.0;
    let top_left_y = origin.1 - hotspot.1;

    let mut start_x = clamp_i32(top_left_x, 0, backend.width as i32);
    let mut start_y = clamp_i32(top_left_y, 0, backend.height as i32);
    let mut end_x = clamp_i32(top_left_x + sprite.width as i32, 0, backend.width as i32);
    let mut end_y = clamp_i32(top_left_y + sprite.height as i32, 0, backend.height as i32);

    if let Some(c) = clip {
        start_x = max(start_x, c.x);
        start_y = max(start_y, c.y);
        end_x = min(end_x, c.x + c.width as i32);
        end_y = min(end_y, c.y + c.height as i32);
    }

    if start_x >= end_x || start_y >= end_y {
        return;
    }

    let sprite_data = &sprite.pixels;
    let sprite_width = sprite.width;

    for y in start_y..end_y {
        let sy = (y - top_left_y) as usize;
        let sx_start = (start_x - top_left_x) as usize;
        let sx_end = (end_x - top_left_x) as usize;
        let sprite_row = &sprite_data[sy * sprite_width + sx_start..sy * sprite_width + sx_end];

        let dst_row_start = (y as usize) * backend.width + (start_x as usize);
        let dst_row =
            &mut backend.storage[dst_row_start..dst_row_start + (end_x - start_x) as usize];

        for (px, dst) in sprite_row.iter().zip(dst_row.iter_mut()) {
            let px = *px;
            let alpha = (px >> 24) & 0xFF;
            if alpha == 0 {
                continue;
            }

            let out = if alpha == 0xFF {
                px
            } else {
                let inv_a = 255 - alpha;
                let dst_val = *dst;
                let dst_r = (dst_val >> 16) & 0xFF;
                let dst_g = (dst_val >> 8) & 0xFF;
                let dst_b = dst_val & 0xFF;

                let src_r = (px >> 16) & 0xFF;
                let src_g = (px >> 8) & 0xFF;
                let src_b = px & 0xFF;

                let r = src_r + (dst_r * inv_a) / 255;
                let g = src_g + (dst_g * inv_a) / 255;
                let b = src_b + (dst_b * inv_a) / 255;

                0xFF000000 | (r << 16) | (g << 8) | b
            };
            *dst = out;
        }
    }
}

fn raster_draw_line(
    backend: &mut BitmapRenderer,
    start: (i32, i32),
    end: (i32, i32),
    color: Rgba,
    clip: Option<Rect>,
) {
    if backend.width == 0 || backend.height == 0 {
        return;
    }

    let x0 = start.0;
    let y0 = start.1;
    let x1 = end.0;
    let y1 = end.1;

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut x = x0;
    let mut y = y0;

    let color_u32 = color.to_u32();

    loop {
        if x >= 0 && x < backend.width as i32 && y >= 0 && y < backend.height as i32 {
            let mut visible = true;
            if let Some(c) = clip {
                if x < c.x || x >= c.x + c.width as i32 || y < c.y || y >= c.y + c.height as i32 {
                    visible = false;
                }
            }

            if visible {
                let idx = y as usize * backend.width + x as usize;
                if color.a == 0xFF {
                    backend.storage[idx] = color_u32;
                } else {
                    let bg = backend.storage[idx];
                    backend.storage[idx] = userland::graphics::blend(color_u32, bg);
                }
            }
        }

        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

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
        let bmp = Bitmap::new(2, 2, vec![0xFF000001, 0xFF000002, 0xFF000003, 0xFF000004]);

        raster_blit_image(
            &mut backend,
            &Rect::new(0, 0, 3, 3),
            &bmp,
            false,
            (0, 0),
            None,
        );

        assert_eq!(
            backend.storage,
            &[0xFF000001, 0xFF000002, 0, 0xFF000003, 0xFF000004, 0, 0, 0, 0]
        );
    }

    #[test]
    fn raster_fill_rect_without_alpha_obeys_clip() {
        let mut backend = BitmapRenderer::new(3, 3);
        backend.storage.fill(0x11111111);

        let rect = Rect::new(1, 1, 3, 3);
        let clip = Some(Rect::new(1, 0, 1, 3));
        let color = SCROLLBAR_TRACK_COLOR;

        raster_fill_rect(&mut backend, &rect, color, clip);

        let mut expected = vec![0x11111111; 9];
        expected[4] = color.to_u32();
        expected[7] = color.to_u32();
        assert_eq!(backend.storage, expected);
    }

    #[test]
    fn raster_fill_rect_with_alpha_and_clipping() {
        let mut backend = BitmapRenderer::new(4, 4);
        backend.storage.fill(0xFFFFFFFF);

        let rect = Rect::new(0, 0, 4, 4);
        let color = SCROLLBAR_THUMB_COLOR.with_alpha(128);
        let clip = Some(Rect::new(1, 1, 2, 2));

        raster_fill_rect(&mut backend, &rect, color, clip);

        let mut expected = vec![0xFFFFFFFF; 16];
        let blended = 0xFFFF7F7F;
        expected[5] = blended;
        expected[6] = blended;
        expected[9] = blended;
        expected[10] = blended;
        assert_eq!(backend.storage, expected);
    }

    #[test]
    fn raster_blit_image_with_repeat_and_offset_wraps() {
        let mut backend = BitmapRenderer::new(4, 4);
        let bmp = Bitmap::new(2, 2, vec![0xFFFF0000, 0xFF00FF00, 0xFF0000FF, 0xFFFFFFFF]);

        raster_blit_image(
            &mut backend,
            &Rect::new(0, 0, 4, 4),
            &bmp,
            true,
            (1, 1),
            None,
        );

        let expected = vec![
            0xFFFFFFFF, 0xFF0000FF, 0xFFFFFFFF, 0xFF0000FF, 0xFF00FF00, 0xFFFF0000, 0xFF00FF00,
            0xFFFF0000, 0xFFFFFFFF, 0xFF0000FF, 0xFFFFFFFF, 0xFF0000FF, 0xFF00FF00, 0xFFFF0000,
            0xFF00FF00, 0xFFFF0000,
        ];
        assert_eq!(backend.storage, expected);
    }

    #[test]
    fn raster_draw_text_block_wraps_and_scrolls_without_overflow() {
        let mut backend = BitmapRenderer::new(16, 32);
        let rect = Rect::new(0, 0, 10, 32);
        let text = "\u{2588}\u{2588}";
        let color = SCROLLBAR_THUMB_HILIGHT;

        raster_draw_text_block(&mut backend, &rect, text, color, FONT_HEIGHT as i32, None);

        let visible_line_has_pixels = (0..FONT_HEIGHT).any(|y| {
            backend.storage[y * backend.width..y * backend.width + rect.width as usize]
                .iter()
                .any(|px| *px != 0)
        });
        assert!(
            visible_line_has_pixels,
            "wrapped glyph should render after scroll"
        );

        for y in 0..FONT_HEIGHT {
            for x in rect.width as usize..backend.width {
                assert_eq!(
                    backend.storage[y * backend.width + x],
                    0,
                    "pixels should not overflow rect width"
                );
            }
        }

        for y in FONT_HEIGHT..backend.height {
            let row_start = y * backend.width;
            let row = &backend.storage[row_start..row_start + backend.width];
            assert!(
                row.iter().all(|px| *px == 0),
                "scrolled-off content should not draw below the first visible line"
            );
        }
    }

    #[test]
    fn present_partial_inside_bounds() {
        let mut storage = vec![0u32; 4 * 3];
        let mut device = BitmapFramebufferDevice::new(4, 3, 4 * 4, storage.as_mut_ptr());

        let frame: Vec<u32> = (0..12).collect();
        let dirty = Rect::new(1, 1, 2, 1);

        device.present_partial(&frame, dirty);

        let expected = vec![0, 0, 0, 0, 0, 5, 6, 0, 0, 0, 0, 0];
        assert_eq!(storage, expected);
    }

    #[test]
    fn present_partial_clipping() {
        let mut storage = vec![0u32; 4 * 4];
        let mut device = BitmapFramebufferDevice::new(4, 4, 4 * 4, storage.as_mut_ptr());

        let shadow = SCROLLBAR_THUMB_SHADOW.to_u32();
        let frame = vec![shadow; 4 * 4];
        let dirty = Rect::new(-1, -1, 3, 3);

        device.present_partial(&frame, dirty);

        let expected = vec![
            shadow, shadow, 0, 0, shadow, shadow, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(storage, expected);
    }
}
